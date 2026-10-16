import fs from "node:fs/promises";
import path from "node:path";
const playwright = await import(process.env.PLAYWRIGHT_PATH ? path.join(process.env.PLAYWRIGHT_PATH, "index.mjs") : "playwright");
const sharpModule = await import(process.env.SHARP_PATH ? path.join(process.env.SHARP_PATH, "dist", "index.mjs") : "sharp");
const { chromium } = playwright;
const sharp = sharpModule.default;

const output = process.env.TINDERBOX_OUTPUT_DIR ?? "tmp/mobile-toolbar-fit-smoke";
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
const normalContext = await browser.newContext({
  viewport: { width: 320, height: 640 },
  deviceScaleFactor: 1,
  isMobile: true,
  hasTouch: true,
});
try {
  const normalPage = await normalContext.newPage();
  await normalPage.goto(baseUrl, { waitUntil: "load", timeout: 60_000 });
  await normalPage.waitForFunction(
    () => ["webgl", "webgpu"].includes(document.querySelector("#the_canvas_id")?.getAttribute("data-renderer")),
    null,
    { timeout: 60_000 },
  );
  await normalPage.waitForTimeout(300);
  const exposed = await normalPage.locator("#the_canvas_id").evaluate((canvas) =>
    ["brush", "cell-size", "wind-strength"].some((name) => canvas.hasAttribute(`data-toolbar-${name}`)),
  );
  if (exposed) throw new Error("normal mode: toolbar test attributes must be disabled");
} finally {
  await normalContext.close();
}
const mobileScenarios = [
  [390, 844, "portrait-390x844"],
  [393, 852, "portrait-393x852"],
  [320, 640, "narrow"],
];
const expectedMobileScenarios = new Set(["portrait-390x844", "portrait-393x852", "narrow"]);
const actualMobileScenarios = new Set(mobileScenarios.map(([, , name]) => name));
if (actualMobileScenarios.size !== expectedMobileScenarios.size ||
    [...expectedMobileScenarios].some((name) => !actualMobileScenarios.has(name))) {
  throw new Error(`Chromium mobile viewport coverage changed unexpectedly: ${mobileScenarios.map(([, , name]) => name).join(", ")}`);
}
const isPhonePortrait = (width, height) => width <= 420 && height >= 800;
if (mobileScenarios.filter(([width, height]) => isPhonePortrait(width, height)).length !== 2) {
  throw new Error("expected both phone portrait scenarios to exercise compact layout and HUD assertions");
}
let portraitAssertionsRun = 0;
let narrowInteractionsRun = 0;
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
  for (const [width, height, name] of mobileScenarios) {
    const phonePortrait = isPhonePortrait(width, height);
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
    testUrl.searchParams.set("safe-area-test", "1");
    await page.addInitScript(() => {
      if (new URLSearchParams(location.search).has("safe-area-test")) {
        document.addEventListener("DOMContentLoaded", () => {
          const style = document.createElement("style");
          style.textContent = ":root { --safe-area-top: 24px !important; --safe-area-right: 0px !important; --safe-area-bottom: 20px !important; --safe-area-left: 0px !important; }";
          document.documentElement.append(style);
        }, { once: true });
      }
    });
    await page.goto(testUrl.toString(), { waitUntil: "load", timeout: 60_000 });
    await page.waitForFunction(
      () => ["webgl", "webgpu"].includes(document.querySelector("#the_canvas_id")?.getAttribute("data-renderer")),
      null,
      { timeout: 60_000 },
    );
    await page.waitForTimeout(500);
    await page.touchscreen.tap(...(width === 320 ? [38, 197] : width === 844 ? [45, 188] : [45, 205]));
    await page.waitForTimeout(500);
    await page.screenshot({ path: `${output}/${name}-compact-toolbar.png` });
    const dimensions = await page.evaluate(() => ({
      viewport: document.documentElement.clientWidth,
      page: document.documentElement.scrollWidth,
      canvas: document.querySelector("#the_canvas_id").getBoundingClientRect().width,
      canvasRect: document.querySelector("#the_canvas_id").getBoundingClientRect().toJSON(),
      safeTop: getComputedStyle(document.documentElement).getPropertyValue("--safe-area-top").trim(),
      safeBottom: getComputedStyle(document.documentElement).getPropertyValue("--safe-area-bottom").trim(),
      toolbar: document.querySelector("#the_canvas_id").getAttribute("data-layout-toolbar")?.split(",").map(Number),
      palette: document.querySelector("#the_canvas_id").getAttribute("data-layout-palette")?.split(",").map(Number),
      world: document.querySelector("#the_canvas_id").getAttribute("data-layout-world")?.split(",").map(Number),
      viewportMeta: document.querySelector('meta[name="viewport"]')?.content,
      canvasTop: getComputedStyle(document.querySelector("#the_canvas_id")).top,
    }));
    if (dimensions.page > dimensions.viewport || dimensions.canvas > dimensions.viewport) {
      throw new Error(`${name}: horizontal overflow ${JSON.stringify(dimensions)}`);
    }
    if (!dimensions.toolbar || !dimensions.palette || !dimensions.world) throw new Error(`${name}: missing panel geometry`);
    if (dimensions.safeTop !== "24px" || dimensions.safeBottom !== "20px" ||
        dimensions.canvasRect.top !== 24 || dimensions.canvasRect.bottom !== height - 20) {
      throw new Error(`${name}: canvas does not respect test safe-area insets: ${JSON.stringify(dimensions)}`);
    }
    const toolbarScreenTop = dimensions.canvasRect.top + dimensions.toolbar[1];
    if (Math.abs(toolbarScreenTop - 24) > 1.5) {
      throw new Error(`${name}: toolbar screen-space top misses the top safe-area inset: ${JSON.stringify({ toolbarScreenTop, dimensions })}`);
    }
    if (Math.abs(dimensions.world[0] - dimensions.palette[2]) > 1.5) {
      throw new Error(`${name}: unexplained palette-to-world gutter: ${JSON.stringify({ palette: dimensions.palette, world: dimensions.world })}`);
    }
    if (Math.abs(dimensions.world[3] - (dimensions.canvasRect.bottom - dimensions.canvasRect.top)) > 2) {
      throw new Error(`${name}: world does not fit to the bottom safe-area edge: ${JSON.stringify({ world: dimensions.world, canvasRect: dimensions.canvasRect })}`);
    }
    const toolbarHeight = dimensions.toolbar[3] - dimensions.toolbar[1];
    const paletteWidth = dimensions.palette[2] - dimensions.palette[0];
    if (phonePortrait) {
      portraitAssertionsRun++;
      if (toolbarHeight > 100 || paletteWidth / width >= 0.22) {
        throw new Error(`${name}: expanded top-level layout exceeds compact geometry: ${JSON.stringify({ toolbarHeight, paletteWidth, width })}`);
      }
    }
    if (!dimensions.viewportMeta?.includes("viewport-fit=cover")) {
      throw new Error(`${name}: viewport-fit=cover is missing`);
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
    if (width <= 420) {
      await page.waitForFunction(() => document.querySelector("#the_canvas_id")?.hasAttribute("data-toolbar-more"));
      const moreBounds = await readControl("more");
      await page.touchscreen.tap((moreBounds.x0 + moreBounds.x1) / 2, dimensions.canvasRect.top + (moreBounds.y0 + moreBounds.y1) / 2);
      await page.waitForFunction(() => {
        const bounds = document.querySelector("#the_canvas_id")?.getAttribute("data-toolbar-more");
        return bounds?.split(",")[8] === "true";
      });
    }
    await page.waitForFunction(() => {
      const canvas = document.querySelector("#the_canvas_id");
      return canvas?.hasAttribute("data-toolbar-brush") &&
        canvas?.hasAttribute("data-toolbar-cell-size") &&
        canvas?.hasAttribute("data-toolbar-wind-strength");
    });
    const hudBounds = await page.locator("#the_canvas_id").getAttribute("data-layout-hud");
    if (hudBounds) {
      const [,, hudRight, hudBottom] = hudBounds.split(",").map(Number);
      const hudTop = Number(hudBounds.split(",")[1]);
      if (hudRight > width + 0.5 || (phonePortrait && hudBottom - hudTop > 80)) {
        throw new Error(`${name}: HUD text is overflowing or too tall: ${hudBounds}`);
      }
    }
    if (phonePortrait && !hudBounds) throw new Error(`${name}: missing wrapped HUD bounds`);
    const result = {
      name,
      width,
      height,
      dimensions,
      hudBounds,
      brushBounds: await readControl("brush"),
      cellSizeBounds: await readControl("cell-size"),
      windStrengthBounds: await readControl("wind-strength"),
      errors,
    };
    if (name === "narrow") {
      narrowInteractionsRun++;
      const speedBounds = await readControl("speed");
      const speedY = dimensions.canvasRect.top + (speedBounds.y0 + speedBounds.y1) / 2;
      await page.mouse.move(speedBounds.x0 + 40, speedY);
      await page.mouse.down();
      await page.mouse.move(speedBounds.x0 + 100, speedY, { steps: 5 });
      await page.mouse.up();
      await page.waitForFunction((previous) => {
        const current = document.querySelector("#the_canvas_id")?.getAttribute("data-toolbar-speed");
        return current && current.split(",")[8] !== previous;
      }, speedBounds.value);
      result.speedTouch = { before: speedBounds.value, after: (await readControl("speed")).value };

      for (const attribute of ["brush", "cell-size", "wind-strength"]) {
        const before = await readControl(attribute);
        await page.touchscreen.tap((before.x0 + before.x1) / 2, dimensions.canvasRect.top + (before.y0 + before.y1) / 2);
        await page.waitForFunction(({ attribute, previous }) => {
          const current = document.querySelector("#the_canvas_id")?.getAttribute(`data-toolbar-${attribute}`);
          return current && current.split(",")[8] !== previous;
        }, { attribute, previous: before.value });
      }
      const toolbarShot = (path) => page.screenshot({
        path,
        clip: { x: 0, y: 0, width: 320, height: 330 },
      });
      const rulesBounds = await readControl("rules");
      const rulesRegion = {
        x: Math.max(0, Math.floor(rulesBounds.x0)),
        y: Math.max(0, Math.floor(dimensions.canvasRect.top + rulesBounds.y0)),
        width: Math.ceil(rulesBounds.x1 - rulesBounds.x0),
        height: Math.ceil(rulesBounds.y1 - rulesBounds.y0),
      };
      const beforeRules = await page.screenshot({
        path: `${output}/before-rules-control.png`,
        clip: rulesRegion,
      });
      await page.touchscreen.tap((rulesBounds.x0 + rulesBounds.x1) / 2, dimensions.canvasRect.top + (rulesBounds.y0 + rulesBounds.y1) / 2);
      await page.waitForTimeout(250);
      await toolbarShot(`${output}/after-rules.png`);
      const afterRules = await page.screenshot({
        path: `${output}/after-rules-control.png`,
        clip: rulesRegion,
      });
      result.rulesCloseRegionChangedPixels = await changedPixels(beforeRules, afterRules);
      if (result.rulesCloseRegionChangedPixels < 20) {
        throw new Error("narrow: Rules was not reachable by touch");
      }
    }
    if (errors.length) throw new Error(`${name}: ${errors.join("\n")}`);
    results.push(result);
    await context.close();
  }
} finally {
  await browser.close();
}
if (portraitAssertionsRun !== 2) {
  throw new Error(`expected compact geometry assertions for both phone portrait cases; ran ${portraitAssertionsRun}`);
}
if (narrowInteractionsRun !== 1) {
  throw new Error(`expected narrow-screen touch interactions to run once; ran ${narrowInteractionsRun}`);
}
await fs.writeFile(`${output}/results.json`, JSON.stringify(results, null, 2));
console.log(JSON.stringify(results, null, 2));
