import fs from "node:fs/promises";
import path from "node:path";
const playwright = await import(process.env.PLAYWRIGHT_PATH ? path.join(process.env.PLAYWRIGHT_PATH, "index.mjs") : "playwright");
const { webkit, devices } = playwright;

const output = process.env.TINDERBOX_OUTPUT_DIR;
if (!output) throw new Error("set TINDERBOX_OUTPUT_DIR to an ignored task evidence directory");
await fs.mkdir(output, { recursive: true });
const baseUrl = process.env.TINDERBOX_TEST_URL ?? "http://127.0.0.1:4173/";
const browser = await webkit.launch({ headless: true });
const normalContext = await browser.newContext({ ...devices["iPhone 13"], viewport: { width: 320, height: 640 } });
try {
  const normalPage = await normalContext.newPage();
  await normalPage.goto(baseUrl, { waitUntil: "load", timeout: 60_000 });
  await normalPage.waitForFunction(
    () => document.querySelector("#the_canvas_id")?.getAttribute("data-renderer") === "webgl",
    null,
    { timeout: 60_000 },
  );
  await normalPage.waitForTimeout(300);
  const exposed = await normalPage.locator("#the_canvas_id").evaluate((canvas) =>
    [...canvas.attributes].some((attribute) => attribute.name.startsWith("data-toolbar-")),
  );
  if (exposed) throw new Error("normal mode: toolbar test attributes must be disabled");
} finally {
  await normalContext.close();
}
const controls = [
  "play-pause", "step", "reset", "navigate", "speed", "brush", "level-setup", "wildfire",
  "cell-size", "hud", "inspect", "overlay", "rules", "wind-west", "wind-calm",
  "wind-east", "wind-strength", "gust",
];
const scenarios = [
  ["portrait-390x664", { ...devices["iPhone 13"], viewport: { width: 390, height: 664 } }, [45, 205]],
  ["portrait-390x844", { ...devices["iPhone 13"], viewport: { width: 390, height: 844 } }, [45, 205]],
  ["landscape-750x342", { ...devices["iPhone 13"], viewport: { width: 750, height: 342 } }, [45, 188]],
  ["narrow-320x640", { ...devices["iPhone 13"], viewport: { width: 320, height: 640 } }, [38, 197]],
  ["desktop-1180x800", { viewport: { width: 1180, height: 800 }, deviceScaleFactor: 1 }, [60, 150]],
];
const results = [];
try {
  for (const [name, config, setupTap] of scenarios) {
    const context = await browser.newContext(config);
    const page = await context.newPage();
    const errors = [];
    page.on("pageerror", (error) => errors.push(error.message));
    page.on("console", (message) => {
      if (message.type() === "error") errors.push(message.text());
    });
    const url = new URL(baseUrl);
    url.searchParams.set("toolbar-test", "1");
    await page.goto(url.toString(), { waitUntil: "load", timeout: 60_000 });
    await page.waitForFunction(
      () => document.querySelector("#the_canvas_id")?.getAttribute("data-renderer") === "webgl",
      null,
      { timeout: 60_000 },
    );
    await page.waitForTimeout(500);
    await page.screenshot({ path: path.join(output, `${name}-setup.png`) });
    if (config.hasTouch) await page.touchscreen.tap(...setupTap);
    else await page.mouse.click(...setupTap);
    await page.waitForTimeout(700);
    const snapshot = await page.evaluate(() => {
      const canvas = document.querySelector("#the_canvas_id");
      return {
        viewport: { width: innerWidth, height: innerHeight, clientWidth: document.documentElement.clientWidth, scrollWidth: document.documentElement.scrollWidth },
        screen: { width: screen.width, height: screen.height },
        touch: { maxTouchPoints: navigator.maxTouchPoints },
        canvas: { ...canvas.getBoundingClientRect().toJSON(), renderer: canvas.getAttribute("data-renderer") },
        testBounds: Object.fromEntries([...canvas.attributes].filter((attribute) => attribute.name.startsWith("data-toolbar-")).map((attribute) => [attribute.name.slice("data-toolbar-".length), attribute.value])),
      };
    });
    const screenshot = path.join(output, `${name}-toolbar.png`);
    await page.screenshot({ path: screenshot });
    if (snapshot.viewport.scrollWidth > snapshot.viewport.width || snapshot.canvas.width > snapshot.viewport.width) {
      throw new Error(`${name}: horizontal overflow ${JSON.stringify(snapshot.viewport)}`);
    }
    if (snapshot.testBounds["play-pause"]?.split(",")[8] !== "true") {
      throw new Error(`${name}: Play level tap did not start the world toolbar`);
    }
    const requiredControls = controls.filter((control) => config.hasTouch || control !== "navigate");
    for (const control of requiredControls) {
      const encoded = snapshot.testBounds[control];
      if (!encoded) throw new Error(`${name}: missing ${control} response bounds`);
      const values = encoded.split(",").map(Number);
      if (values.length !== 9) throw new Error(`${name}: malformed ${control} response bounds: ${encoded}`);
      const [x0, y0, x1, y1, clipX0, clipY0, clipX1, clipY1] = values;
      const insideClip = x0 >= clipX0 - 0.5 && y0 >= clipY0 - 0.5 && x1 <= clipX1 + 0.5 && y1 <= clipY1 + 0.5;
      const insideViewport = x0 >= 0 && y0 >= 0 && x1 <= snapshot.viewport.width + 0.5 && y1 <= snapshot.viewport.height + 0.5;
      const touchSized = !config.hasTouch || (x1 - x0 >= 44 && y1 - y0 >= 44);
      if (!insideClip || !insideViewport || !touchSized) {
        throw new Error(`${name}: ${control} clipped or undersized: ${JSON.stringify({ x0, y0, x1, y1, clipX0, clipY0, clipX1, clipY1, insideViewport, touchSized })}`);
      }
    }
    if (errors.length) throw new Error(`${name}: browser errors: ${errors.join("\n")}`);
    const result = { name, viewport: snapshot.viewport, screen: snapshot.screen, touch: { descriptorHasTouch: Boolean(config.hasTouch), maxTouchPoints: snapshot.touch.maxTouchPoints }, controls: Object.fromEntries(requiredControls.map((control) => [control, snapshot.testBounds[control]])), errors, screenshot };
    results.push(result);
    console.log(JSON.stringify(result));
    await context.close();
  }
} finally {
  await browser.close();
}
await fs.writeFile(path.join(output, "results.json"), JSON.stringify(results, null, 2));
