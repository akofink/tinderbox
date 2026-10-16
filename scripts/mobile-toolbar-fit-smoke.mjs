import { chromium } from "playwright";
import fs from "node:fs/promises";
import sharp from "sharp";

const output = "tmp/mobile-toolbar-fit-smoke";
const baseUrl = process.env.TINDERBOX_TEST_URL ?? "http://127.0.0.1:4173/";
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
const results = [];
const changedPixels = async (a, b) => {
  const [left, right] = await Promise.all([sharp(a).raw().toBuffer(), sharp(b).raw().toBuffer()]);
  let changed = 0;
  for (let index = 0; index < left.length; index += 3) {
    if (left[index] !== right[index] || left[index + 1] !== right[index + 1] || left[index + 2] !== right[index + 2]) {
      changed++;
    }
  }
  return changed;
};
try {
  for (const [width, height, name] of [
    [390, 844, "portrait"],
    [844, 390, "landscape"],
    [320, 640, "narrow"],
  ]) {
    const context = await browser.newContext({
      viewport: { width, height },
      deviceScaleFactor: 1,
      isMobile: true,
      hasTouch: true,
    });
    const page = await context.newPage();
    const errors = [];
    page.on("pageerror", (error) => errors.push(error.message));
    page.on("console", (message) => {
      if (message.type() === "error") errors.push(message.text());
    });
    await page.goto(baseUrl, { waitUntil: "load", timeout: 60_000 });
    await page.waitForFunction(
      () => document.querySelector("#the_canvas_id")?.getAttribute("data-renderer") === "webgl",
      null,
      { timeout: 60_000 },
    );
    await page.waitForTimeout(700);
    const dimensions = await page.evaluate(() => ({
      viewport: document.documentElement.clientWidth,
      page: document.documentElement.scrollWidth,
      canvas: document.querySelector("#the_canvas_id").getBoundingClientRect().width,
    }));
    if (dimensions.page > dimensions.viewport || dimensions.canvas > dimensions.viewport) {
      throw new Error(`${name}: horizontal overflow ${JSON.stringify(dimensions)}`);
    }
    const result = { name, width, height, dimensions, errors };
    if (name === "narrow") {
      await page.touchscreen.tap(38, 197);
      await page.waitForTimeout(500);
      await page.touchscreen.tap(34, 23);
      await page.waitForTimeout(250);
      const toolbarShot = (path) => page.screenshot({
        path,
        clip: { x: 0, y: 0, width: 320, height: 330 },
      });
      const beforeRules = await page.screenshot({
        path: `${output}/before-rules-close-region.png`,
        clip: { x: 293, y: 76, width: 27, height: 24 },
      });
      await page.touchscreen.tap(201, 155);
      await page.waitForTimeout(250);
      await toolbarShot(`${output}/after-rules.png`);
      const afterRules = await page.screenshot({
        path: `${output}/after-rules-close-region.png`,
        clip: { x: 293, y: 76, width: 27, height: 24 },
      });
      result.rulesCloseRegionChangedPixels = await changedPixels(beforeRules, afterRules);
      if (result.rulesCloseRegionChangedPixels < 20) {
        throw new Error("narrow: Rules was not reachable by touch");
      }

      await page.touchscreen.tap(304, 88);
      await page.waitForTimeout(200);
      const beforeSpeed = await toolbarShot(`${output}/before-speed.png`);
      await page.touchscreen.tap(128, 70);
      await page.waitForTimeout(200);
      const afterSpeed = await toolbarShot(`${output}/after-speed.png`);
      result.speedChangedPixels = await changedPixels(beforeSpeed, afterSpeed);
      if (result.speedChangedPixels < 20) throw new Error("narrow: Speed slider was not reachable by touch");

      const beforeWind = await toolbarShot(`${output}/before-wind-strength.png`);
      await page.touchscreen.tap(145, 306);
      await page.waitForTimeout(200);
      const afterWind = await toolbarShot(`${output}/after-wind-strength.png`);
      result.windStrengthChangedPixels = await changedPixels(beforeWind, afterWind);
      if (result.windStrengthChangedPixels < 20) {
        throw new Error("narrow: Wind-strength slider was not reachable by touch");
      }
    }
    if (errors.length) throw new Error(`${name}: ${errors.join("\n")}`);
    results.push(result);
    await context.close();
  }
} finally {
  await browser.close();
}
await fs.writeFile(`${output}/results.json`, JSON.stringify(results, null, 2));
console.log(JSON.stringify(results, null, 2));
