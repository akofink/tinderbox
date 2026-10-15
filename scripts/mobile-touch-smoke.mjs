import { chromium } from "playwright";
import fs from "node:fs/promises";

const errors = [];
const output = "tmp/mobile-touch-smoke";
await fs.mkdir(output, { recursive: true });
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
  const context = await browser.newContext({
    viewport: { width: 390, height: 844 },
    deviceScaleFactor: 1,
    isMobile: true,
    hasTouch: true,
  });
  const page = await context.newPage();
  page.on("pageerror", (error) => errors.push(`pageerror: ${error.message}`));
  page.on("console", (message) => {
    if (message.type() === "error") errors.push(`console: ${message.text()}`);
  });
  await page.goto("http://127.0.0.1:4173/", { waitUntil: "load", timeout: 60_000 });
  const canvas = page.locator("#the_canvas_id");
  await page.waitForFunction(() => {
    const target = document.querySelector("#the_canvas_id");
    return target?.getAttribute("data-renderer") && target.getAttribute("data-frame-ms");
  }, null, { timeout: 60_000 });

  // Dismiss the initial level setup and use the visible touch-sized Inspect control.
  await page.touchscreen.tap(40, 179);
  await page.waitForTimeout(500);
  await page.touchscreen.tap(110, 84);
  await page.waitForTimeout(250);
  await page.touchscreen.tap(250, 400);
  await page.waitForTimeout(250);
  await page.screenshot({ path: `${output}/inspected.png` });
  await page.touchscreen.tap(110, 84);
  await page.waitForTimeout(250);
  await page.screenshot({ path: `${output}/dismissed.png` });

  if (errors.length) throw new Error(errors.join("\n"));
  console.log("Mobile touch Inspector smoke passed at 390x844 (touch enabled)");
  await context.close();

  const narrowContext = await browser.newContext({
    viewport: { width: 320, height: 640 },
    deviceScaleFactor: 1,
    isMobile: true,
    hasTouch: true,
  });
  const narrowPage = await narrowContext.newPage();
  await narrowPage.goto("http://127.0.0.1:4173/", { waitUntil: "load", timeout: 60_000 });
  await narrowPage.waitForFunction(() => {
    const target = document.querySelector("#the_canvas_id");
    return target?.getAttribute("data-renderer") && target.getAttribute("data-frame-ms");
  }, null, { timeout: 60_000 });
  await narrowPage.waitForTimeout(1500);
  await narrowPage.touchscreen.tap(40, 132);
  await narrowPage.waitForTimeout(500);
  await narrowPage.screenshot({ path: `${output}/narrow.png` });
  await narrowContext.close();
} finally {
  await browser.close();
}
