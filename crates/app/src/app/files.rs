#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
//! Text save/load that stays wasm-friendly.
//!
//! Native code reads and writes a path. The browser build stores `tinderbox:<key>` in
//! `localStorage`, downloads a file, and accepts an upload or a dropped file. UI code
//! should use this module instead of `std::fs`.

use std::sync::Mutex;

const MAX_BYTES: usize = 65_536;

static NOTICE: Mutex<String> = Mutex::new(String::new());

pub(crate) fn set_notice(text: impl Into<String>) {
    if let Ok(mut notice) = NOTICE.lock() {
        *notice = text.into();
    }
}

pub(crate) fn notice() -> String {
    NOTICE
        .lock()
        .map(|notice| notice.clone())
        .unwrap_or_default()
}

pub(crate) fn save_text(key: &str, filename: &str, contents: &str) -> Result<(), String> {
    check_size(contents)?;
    #[cfg(target_arch = "wasm32")]
    {
        remember(key, contents)?;
        download(filename, contents)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = key;
        std::fs::write(filename, contents).map_err(|err| format!("Cannot save {filename}: {err}"))
    }
}

pub(crate) fn load_text(key: &str) -> Result<String, String> {
    #[cfg(target_arch = "wasm32")]
    {
        load_stored(key)?.ok_or_else(|| format!("No stored {key} in this browser."))
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::io::Read;
        let file = std::fs::File::open(key).map_err(|err| format!("Cannot load {key}: {err}"))?;
        let mut text = String::new();
        file.take((MAX_BYTES + 1) as u64)
            .read_to_string(&mut text)
            .map_err(|err| format!("Cannot read {key}: {err}"))?;
        check_size(&text)?;
        Ok(text)
    }
}

pub(crate) fn remember(key: &str, contents: &str) -> Result<(), String> {
    check_size(contents)?;
    #[cfg(target_arch = "wasm32")]
    {
        storage()?
            .set_item(&storage_key(key), contents)
            .map_err(|err| format!("Cannot store {key}: {err:?}"))
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::fs::write(key, contents).map_err(|err| format!("Cannot save {key}: {err}"))
    }
}

fn check_size(contents: &str) -> Result<(), String> {
    if contents.len() > MAX_BYTES {
        Err("File is too large (maximum 64 KiB)".into())
    } else {
        Ok(())
    }
}

fn storage_key(key: &str) -> String {
    format!("tinderbox:{key}")
}

#[cfg(target_arch = "wasm32")]
mod browser {
    use super::{check_size, storage_key};
    use std::cell::RefCell;
    use wasm_bindgen::JsCast;
    use wasm_bindgen::JsValue;
    use wasm_bindgen::closure::Closure;

    type UploadSlot = Option<(String, Result<String, String>)>;
    type EventClosure = Closure<dyn FnMut(web_sys::Event)>;

    thread_local! {
        static UPLOAD: RefCell<UploadSlot> = const { RefCell::new(None) };
        static ONCHANGE: RefCell<Option<EventClosure>> = const { RefCell::new(None) };
        static ONLOAD: RefCell<Option<EventClosure>> = const { RefCell::new(None) };
    }

    pub(super) fn storage() -> Result<web_sys::Storage, String> {
        web_sys::window()
            .ok_or("no browser window")?
            .local_storage()
            .map_err(|err| format!("localStorage unavailable: {err:?}"))?
            .ok_or_else(|| "localStorage unavailable".into())
    }

    pub(super) fn load_stored(key: &str) -> Result<Option<String>, String> {
        storage()?
            .get_item(&storage_key(key))
            .map_err(|err| format!("Cannot read {key}: {err:?}"))
    }

    pub(super) fn download(filename: &str, contents: &str) -> Result<(), String> {
        let window = web_sys::window().ok_or("no browser window")?;
        let document = window.document().ok_or("no document")?;
        let parts = js_sys::Array::new();
        parts.push(&JsValue::from_str(contents));
        let options = web_sys::BlobPropertyBag::new();
        options.set_type("text/plain;charset=utf-8");
        let blob = web_sys::Blob::new_with_str_sequence_and_options(&parts, &options)
            .map_err(|err| format!("Cannot build download: {err:?}"))?;
        let url = web_sys::Url::create_object_url_with_blob(&blob)
            .map_err(|err| format!("Cannot build download: {err:?}"))?;
        let anchor = document
            .create_element("a")
            .map_err(|err| format!("Cannot build download: {err:?}"))?
            .dyn_into::<web_sys::HtmlAnchorElement>()
            .map_err(|err| format!("Cannot build download: {err:?}"))?;
        anchor.set_href(&url);
        anchor.set_download(filename);
        anchor.click();
        let _ = web_sys::Url::revoke_object_url(&url);
        Ok(())
    }

    pub(super) fn begin_upload(key: &str) -> Result<(), String> {
        let document = web_sys::window()
            .ok_or("no browser window")?
            .document()
            .ok_or("no document")?;
        let input = document
            .create_element("input")
            .map_err(|err| format!("Cannot open file picker: {err:?}"))?
            .dyn_into::<web_sys::HtmlInputElement>()
            .map_err(|err| format!("Cannot open file picker: {err:?}"))?;
        input.set_type("file");
        input.set_accept(".toml,text/plain,.txt");
        let key = key.to_owned();
        let onchange = Closure::wrap(Box::new(move |event: web_sys::Event| {
            let Some(input) = event
                .target()
                .and_then(|target| target.dyn_into::<web_sys::HtmlInputElement>().ok())
            else {
                return;
            };
            let Some(file) = input.files().and_then(|files| files.get(0)) else {
                return;
            };
            let Ok(reader) = web_sys::FileReader::new() else {
                finish(key.clone(), Err("Cannot read the uploaded file".into()));
                return;
            };
            let reader_done = reader.clone();
            let done_key = key.clone();
            let onload = Closure::wrap(Box::new(move |_event: web_sys::Event| {
                let text = reader_done
                    .result()
                    .ok()
                    .and_then(|value| value.as_string())
                    .ok_or_else(|| "Uploaded file was not text".to_owned());
                let text = text.and_then(|text| check_size(&text).map(|()| text));
                finish(done_key.clone(), text);
            }) as Box<dyn FnMut(_)>);
            reader.set_onload(Some(onload.as_ref().unchecked_ref()));
            ONLOAD.with(|slot| *slot.borrow_mut() = Some(onload));
            if reader.read_as_text(&file).is_err() {
                finish(key.clone(), Err("Cannot read the uploaded file".into()));
            }
        }) as Box<dyn FnMut(_)>);
        input.set_onchange(Some(onchange.as_ref().unchecked_ref()));
        ONCHANGE.with(|slot| *slot.borrow_mut() = Some(onchange));
        input.click();
        Ok(())
    }

    pub(super) fn finish_upload(key: &str, text: Result<String, String>) {
        finish(key.to_owned(), text);
    }

    fn finish(key: String, text: Result<String, String>) {
        UPLOAD.with(|slot| *slot.borrow_mut() = Some((key, text)));
    }

    pub(super) fn poll(ctx: &eframe::egui::Context) -> Option<(String, Result<String, String>)> {
        let done = UPLOAD.with(|slot| slot.borrow_mut().take());
        if done.is_some() {
            return done;
        }
        dropped_text(ctx).map(|text| ("rules".to_owned(), check_size(&text).map(|()| text)))
    }

    fn dropped_text(ctx: &eframe::egui::Context) -> Option<String> {
        ctx.input(|input| {
            input.raw.dropped_files.iter().find_map(|file| {
                file.bytes
                    .as_ref()
                    .and_then(|bytes| String::from_utf8(bytes.to_vec()).ok())
            })
        })
    }
}

#[cfg(target_arch = "wasm32")]
use browser::{
    begin_upload as begin_upload_browser, download, finish_upload as finish_upload_browser,
    load_stored, poll as poll_browser, storage,
};

#[cfg(target_arch = "wasm32")]
pub(crate) fn begin_upload(key: &str) -> Result<(), String> {
    begin_upload_browser(key)
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn finish_upload(key: &str, text: Result<String, String>) {
    finish_upload_browser(key, text);
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn poll(ctx: &eframe::egui::Context) -> Option<(String, String)> {
    match poll_browser(ctx)? {
        (key, Ok(text)) => Some((key, text)),
        (_, Err(err)) => {
            set_notice(err);
            None
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::{load_text, notice, remember, save_text, set_notice};

    #[test]
    fn native_round_trip_and_size_limit() {
        let path = std::env::temp_dir().join(format!("tinderbox-files-{}", std::process::id()));
        let path = path.to_string_lossy().into_owned();
        save_text("ignored", &path, "sand = 1\n").unwrap();
        let loaded = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(loaded, "sand = 1\n");
        remember(&path, "soil = 2\n").unwrap();
        assert_eq!(load_text(&path).unwrap(), "soil = 2\n");
        std::fs::remove_file(&path).unwrap();
        let huge = "x".repeat(65_537);
        assert!(
            save_text("ignored", "unused.toml", &huge)
                .unwrap_err()
                .contains("64 KiB")
        );
        set_notice("ok");
        assert_eq!(notice(), "ok");
    }
}
