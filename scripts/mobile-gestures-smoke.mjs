import { chromium } from "playwright";
import fs from "node:fs/promises";

const output = process.env.TINDERBOX_GESTURES_OUTPUT_DIR ?? "tmp/mobile-gestures-smoke";
await fs.mkdir(output, { recursive: true });
const browser = await chromium.launch({
  headless: true,
  args: ["--enable-webgl", "--ignore-gpu-blocklist", "--use-gl=angle", "--use-angle=swiftshader", "--enable-unsafe-swiftshader"],
});
const results = [];
try {
  for (const [name, width, height] of [
    ["portrait", 390, 844],
    ["landscape", 844, 390],
    ["narrow", 320, 640],
  ]) {
    const errors = [];
    const context = await browser.newContext({
      viewport: { width, height }, deviceScaleFactor: 1, isMobile: true, hasTouch: true,
    });
    const page = await context.newPage();
    page.on("pageerror", (error) => errors.push(error.message));
    page.on("console", (message) => { if (message.type() === "error") errors.push(message.text()); });
    const testUrl = new URL(process.env.TINDERBOX_TEST_URL ?? "http://127.0.0.1:4173/");
    testUrl.searchParams.set("profile", "endless");
    testUrl.searchParams.set("toolbar-test", "1");
    await page.goto(testUrl.toString(), { waitUntil: "load", timeout: 60_000 });
    await page.waitForFunction(() => document.querySelector("#the_canvas_id")?.getAttribute("data-renderer"), null, { timeout: 60_000 });
    const canvas = page.locator("#the_canvas_id");
    await page.waitForFunction(() => document.querySelector("#the_canvas_id")?.hasAttribute("data-toolbar-play-pause"));
    await page.waitForTimeout(1_000);
    const controlBounds = async (name) => {
      const encoded = await canvas.getAttribute(`data-toolbar-${name}`);
      if (!encoded) throw new Error(`missing ${name} toolbar bounds`);
      const [x0, y0, x1, y1] = encoded.split(",").slice(0, 4).map(Number);
      const rect = await canvas.boundingBox();
      return { x: rect.x + (x0 + x1) / 2, y: rect.y + (y0 + y1) / 2 };
    };
    const tapControl = async (name) => {
      const point = await controlBounds(name);
      await page.touchscreen.tap(point.x, point.y);
    };
    await tapControl("play-pause"); // Pause simulation so screenshots are stable.
    await page.waitForTimeout(350);
    await tapControl("navigate");
    await page.waitForTimeout(250);

    const bounds = await canvas.boundingBox();
    const cx = Math.round(bounds.x + bounds.width * 0.72);
    const cy = Math.round(bounds.y + bounds.height * 0.67);
    const session = await context.newCDPSession(page);
    const snap = async (label) => {
      await page.waitForTimeout(300);
      return page.screenshot({ path: `${output}/${name}-${label}.png` });
    };
    const beforeZoom = await snap("before");
    await session.send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [{ x: cx - 35, y: cy - 25 }, { x: cx + 35, y: cy + 25 }] });
    await session.send("Input.dispatchTouchEvent", { type: "touchMove", touchPoints: [{ x: cx - 75, y: cy - 55 }, { x: cx + 75, y: cy + 55 }] });
    await page.waitForTimeout(150);
    await session.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
    const afterZoom = await snap("zoomed");
    const zoomChanged = beforeZoom.reduce((count, byte, index) => count + (byte !== afterZoom[index]), 0);
    if (zoomChanged < 1500) throw new Error(`${name}: real two-finger pinch produced no visible zoom change (${zoomChanged} PNG byte differences)`);

    await session.send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [{ x: cx, y: cy }] });
    await session.send("Input.dispatchTouchEvent", { type: "touchMove", touchPoints: [{ x: cx - 65, y: cy }] });
    await session.send("Input.dispatchTouchEvent", { type: "touchMove", touchPoints: [{ x: cx - 100, y: cy }] });
    await session.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
    const afterPan = await snap("panned");
    const panChanged = afterZoom.reduce((count, byte, index) => count + (byte !== afterPan[index]), 0);
    if (panChanged < 1000) throw new Error(`${name}: one-finger Navigate drag produced no visible camera change (${panChanged} PNG byte differences)`);

    await tapControl("navigate"); // Return to painting.
    const beforePaint = await snap("before-paint");
    await page.touchscreen.tap(cx, cy);
    const afterPaint = await snap("painted");
    const paintChanged = beforePaint.reduce((count, byte, index) => count + (byte !== afterPaint[index]), 0);
    if (paintChanged < 100) throw new Error(`${name}: one-finger touch paint produced no visible change (${paintChanged} PNG byte differences)`);

    if (width <= 420) await tapControl("more");
    await tapControl("inspect");
    const beforeInspect = await snap("before-inspect");
    await page.touchscreen.tap(cx, cy);
    const afterInspect = await snap("inspected");
    const inspectChanged = beforeInspect.reduce((count, byte, index) => count + (byte !== afterInspect[index]), 0);
    if (inspectChanged < 100) throw new Error(`${name}: Inspector tap produced no visible selection (${inspectChanged} PNG byte differences)`);
    await session.send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [{ x: cx, y: cy }] });
    await session.send("Input.dispatchTouchEvent", { type: "touchMove", touchPoints: [{ x: cx - 55, y: cy + 20 }] });
    await session.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
    await snap("inspector-drag");
    if (errors.length) throw new Error(`${name}: browser errors: ${errors.join("; ")}`);
    results.push({ name, zoomChanged, panChanged, paintChanged, inspectChanged, errors });
    await context.close();
  }
} finally {
  await browser.close();
}
await fs.writeFile(`${output}/results.json`, JSON.stringify(results, null, 2));
console.log(JSON.stringify(results, null, 2));
