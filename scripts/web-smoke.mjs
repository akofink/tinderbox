import { chromium } from "playwright";

const errors = [];
const browser = await chromium.launch({
  headless: true,
  args: [
    "--enable-webgl",
    "--ignore-gpu-blocklist",
    "--use-gl=angle",
    "--use-angle=swiftshader",
    "--enable-unsafe-swiftshader",
  ],
});

try {
  const page = await browser.newPage({ viewport: { width: 1180, height: 800 } });
  page.on("pageerror", (error) => errors.push(`pageerror: ${error.message}`));
  page.on("console", (message) => {
    if (message.type() === "error") errors.push(`console: ${message.text()}`);
  });
  await page.goto("http://127.0.0.1:4173/", { waitUntil: "load", timeout: 60_000 });
  const canvas = page.locator("#the_canvas_id");
  await canvas.waitFor({ state: "visible", timeout: 30_000 });
  await page.waitForFunction(() => {
    const target = document.querySelector("#the_canvas_id");
    return target?.getAttribute("data-renderer") && target.getAttribute("data-frame-ms");
  }, null, { timeout: 60_000 });

  await page.evaluate(() => {
    const target = document.querySelector("#the_canvas_id");
    if (!target) throw new Error("canvas is missing");
    window.__tinderboxFrameUpdates = 0;
    new MutationObserver((records) => {
      window.__tinderboxFrameUpdates += records.length;
    }).observe(target, { attributes: true, attributeFilter: ["data-frame-ms"] });
  });
  await page.waitForTimeout(10_000);
  const frameUpdates = await page.evaluate(() => window.__tinderboxFrameUpdates ?? 0);
  if (frameUpdates < 10) errors.push(`too few canvas frame updates: ${frameUpdates}`);
  const bodyText = await page.locator("body").innerText();
  if (/panic|panicked at|unreachable!|Tinderbox failed to start/i.test(bodyText)) {
    errors.push(`panic or startup failure text: ${bodyText}`);
  }
  await canvas.screenshot({ path: "web-smoke-canvas.png" });
  if (errors.length) throw new Error(errors.join("\n"));
  console.log(`Web smoke passed: renderer=${await canvas.getAttribute("data-renderer")}, ${frameUpdates} frame updates in 10 seconds`);
} finally {
  await browser.close();
}
