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
    const testUrl = new URL(baseUrl);
    testUrl.searchParams.set("toolbar-test", "1");
    await page.goto(testUrl.toString(), { waitUntil: "load", timeout: 60_000 });
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
    const readControl = async (attribute) => {
      const encoded = await page.locator("#the_canvas_id").getAttribute(`data-toolbar-${attribute}`);
      if (!encoded) throw new Error(`${name}: missing ${attribute} control bounds`);
      const values = encoded.split(",");
      if (values.length !== 9) throw new Error(`${name}: malformed ${attribute} bounds: ${encoded}`);
      const [x0, y0, x1, y1, clipX0, clipY0, clipX1, clipY1] = values.slice(0, 8).map(Number);
      const value = values[8];
      const bounds = { x0, y0, x1, y1, clipX0, clipY0, clipX1, clipY1, value };
      if (
        x0 < clipX0 - 0.5 || y0 < clipY0 - 0.5 || x1 > clipX1 + 0.5 || y1 > clipY1 + 0.5 ||
        x0 < 0 || y0 < 0 || x1 > dimensions.viewport + 0.5 || y1 > height + 0.5 ||
        x1 - x0 < 44 || y1 - y0 < 44
      ) {
        throw new Error(`${name}: ${attribute} control is clipped or undersized: ${JSON.stringify(bounds)}`);
      }
      return bounds;
    };
    await page.waitForFunction(() => {
      const canvas = document.querySelector("#the_canvas_id");
      return canvas?.hasAttribute("data-toolbar-brush") && canvas?.hasAttribute("data-toolbar-cell-size");
    });
    const result = {
      name,
      width,
      height,
      dimensions,
      brushBounds: await readControl("brush"),
      cellSizeBounds: await readControl("cell-size"),
      errors,
    };
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
      await page.touchscreen.tap(201, 211);
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

      for (const [attribute, filename] of [["brush", "brush"], ["cell-size", "cell-size"]]) {
        const before = await readControl(attribute);
        const x = (before.x0 + before.x1) / 2;
        const y = (before.y0 + before.y1) / 2;
        await page.touchscreen.tap(x, y);
        await page.waitForFunction(
          ({ attribute, previous }) => {
            const current = document.querySelector("#the_canvas_id")?.getAttribute(`data-toolbar-${attribute}`);
            return current && current.split(",")[8] !== previous;
          },
          { attribute, previous: before.value },
        );
        const after = await readControl(attribute);
        result[`${filename}Touch`] = { before: before.value, after: after.value, bounds: after };
      }

      const beforeWind = await toolbarShot(`${output}/before-wind-strength.png`);
      await page.touchscreen.tap(145, 354);
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
