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
    () => ["webgl", "webgpu"].includes(document.querySelector("#the_canvas_id")?.getAttribute("data-renderer")),
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
  ["portrait-390x844", { ...devices["iPhone 13"], viewport: { width: 390, height: 844 } }, [45, 205]],
  ["portrait-393x852", { ...devices["iPhone 13"], viewport: { width: 393, height: 852 } }, [45, 205]],
  ["landscape-750x342", { ...devices["iPhone 13"], viewport: { width: 750, height: 342 } }, [45, 212]],
  ["desktop-1180x800", { viewport: { width: 1180, height: 800 }, deviceScaleFactor: 1 }, [80, 170]],
];
const expectedScenarios = new Set([
  "portrait-390x844", "portrait-393x852", "landscape-750x342", "desktop-1180x800",
]);
const actualScenarioNames = new Set(scenarios.map(([name]) => name));
if (actualScenarioNames.size !== expectedScenarios.size ||
    [...expectedScenarios].some((name) => !actualScenarioNames.has(name))) {
  throw new Error(`WebKit viewport coverage changed unexpectedly: ${scenarios.map(([name]) => name).join(", ")}`);
}
const isPhonePortrait = (config) => config.viewport.width <= 420 && config.viewport.height >= 800;
const results = [];
try {
  for (const [name, config, setupTap] of scenarios) {
    const phonePortrait = isPhonePortrait(config);
    const context = await browser.newContext(config);
    const page = await context.newPage();
    const errors = [];
    page.on("pageerror", (error) => errors.push(error.message));
    page.on("console", (message) => {
      if (message.type() === "error") errors.push(message.text());
    });
    const url = new URL(baseUrl);
    url.searchParams.set("toolbar-test", "1");
    url.searchParams.set("safe-area-test", "1");
    await page.addInitScript(() => {
      if (new URLSearchParams(location.search).has("safe-area-test")) {
        document.addEventListener("DOMContentLoaded", () => {
          const style = document.createElement("style");
          style.textContent = ":root { --safe-area-top: 24px !important; --safe-area-right: 0px !important; --safe-area-bottom: 20px !important; --safe-area-left: 0px !important; }";
          document.documentElement.append(style);
        }, { once: true });
      }
    });
    await page.goto(url.toString(), { waitUntil: "load", timeout: 60_000 });
    await page.waitForFunction(
      () => ["webgl", "webgpu"].includes(document.querySelector("#the_canvas_id")?.getAttribute("data-renderer")),
      null,
      { timeout: 60_000 },
    );
    await page.waitForTimeout(500);
    await page.screenshot({ path: path.join(output, `${name}-setup.png`) });
    if (config.hasTouch) await page.touchscreen.tap(...setupTap);
    else await page.mouse.click(...setupTap);
    await page.waitForTimeout(700);
    await page.screenshot({ path: path.join(output, `${name}-compact-toolbar.png`) });
    const collapsed = await page.evaluate(() => {
      const canvas = document.querySelector("#the_canvas_id");
      return {
        canvasRect: canvas.getBoundingClientRect().toJSON(),
        safeTop: getComputedStyle(document.documentElement).getPropertyValue("--safe-area-top").trim(),
        safeBottom: getComputedStyle(document.documentElement).getPropertyValue("--safe-area-bottom").trim(),
        toolbar: canvas.getAttribute("data-layout-toolbar")?.split(",").map(Number),
        palette: canvas.getAttribute("data-layout-palette")?.split(",").map(Number),
        world: canvas.getAttribute("data-layout-world")?.split(",").map(Number),
        more: canvas.getAttribute("data-toolbar-more")?.split(",").map(Number),
        viewportMeta: document.querySelector('meta[name="viewport"]')?.content,
      };
    });
    if (!collapsed.toolbar || !collapsed.palette || !collapsed.world) throw new Error(`${name}: missing panel geometry`);
    if (collapsed.safeTop !== "24px" || collapsed.safeBottom !== "20px" ||
        collapsed.canvasRect.top !== 24 || collapsed.canvasRect.bottom !== config.viewport.height - 20) {
      throw new Error(`${name}: canvas does not respect test safe-area insets: ${JSON.stringify(collapsed)}`);
    }
    const toolbarHeight = collapsed.toolbar[3] - collapsed.toolbar[1];
    const paletteWidth = collapsed.palette[2] - collapsed.palette[0];
    const toolbarScreenTop = collapsed.canvasRect.top + collapsed.toolbar[1];
    if (Math.abs(toolbarScreenTop - 24) > 1.5) {
      throw new Error(`${name}: toolbar screen-space top misses the top safe-area inset: ${JSON.stringify({ toolbarScreenTop, collapsed })}`);
    }
    if (Math.abs(collapsed.world[0] - collapsed.palette[2]) > 1.5) {
      throw new Error(`${name}: unexplained palette-to-world gutter: ${JSON.stringify({ palette: collapsed.palette, world: collapsed.world })}`);
    }
    if (Math.abs(collapsed.world[3] - (collapsed.canvasRect.bottom - collapsed.canvasRect.top)) > 2) {
      throw new Error(`${name}: world does not fit to the bottom safe-area edge: ${JSON.stringify({ world: collapsed.world, canvasRect: collapsed.canvasRect })}`);
    }
    if (phonePortrait && (toolbarHeight > 100 || paletteWidth / config.viewport.width >= 0.22)) {
      throw new Error(`${name}: compact layout geometry failed: ${JSON.stringify({ toolbarHeight, paletteWidth })}`);
    }
    if (!collapsed.viewportMeta?.includes("viewport-fit=cover")) throw new Error(`${name}: viewport-fit=cover missing`);
    if (config.hasTouch && config.viewport.width <= 420) {
      if (!collapsed.more) throw new Error(`${name}: More controls toggle is missing`);
      const [moreX0, moreY0, moreX1, moreY1] = collapsed.more;
      await page.touchscreen.tap((moreX0 + moreX1) / 2, (moreY0 + moreY1) / 2);
      await page.waitForFunction(() => {
        const bounds = document.querySelector("#the_canvas_id")?.getAttribute("data-toolbar-more");
        return bounds?.split(",")[8] === "true";
      });
    }
    await page.waitForFunction(() => document.querySelector("#the_canvas_id")?.hasAttribute("data-toolbar-wind-strength"));
    const snapshot = await page.evaluate(() => {
      const canvas = document.querySelector("#the_canvas_id");
      return {
        viewport: { width: innerWidth, height: innerHeight, clientWidth: document.documentElement.clientWidth, scrollWidth: document.documentElement.scrollWidth },
        screen: { width: screen.width, height: screen.height },
        touch: { maxTouchPoints: navigator.maxTouchPoints },
        canvas: { ...canvas.getBoundingClientRect().toJSON(), renderer: canvas.getAttribute("data-renderer") },
        testBounds: Object.fromEntries([...canvas.attributes].filter((attribute) => attribute.name.startsWith("data-toolbar-")).map((attribute) => [attribute.name.slice("data-toolbar-".length), attribute.value])),
        hudBounds: canvas.getAttribute("data-layout-hud")?.split(",").map(Number),
      };
    });
    const screenshot = path.join(output, `${name}-toolbar.png`);
    await page.screenshot({ path: screenshot });
    if (snapshot.viewport.scrollWidth > snapshot.viewport.width || snapshot.canvas.width > snapshot.viewport.width) {
      throw new Error(`${name}: horizontal overflow ${JSON.stringify(snapshot.viewport)}`);
    }
    if (phonePortrait && (!snapshot.hudBounds || snapshot.hudBounds[2] > snapshot.viewport.width + 0.5 || snapshot.hudBounds[3] - snapshot.hudBounds[1] > 80)) {
      throw new Error(`${name}: HUD text is missing, overflowing, or too tall: ${JSON.stringify(snapshot.hudBounds)}`);
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
    const result = { name, viewport: snapshot.viewport, screen: snapshot.screen, layout: collapsed, compactGeometry: { toolbarHeight, paletteWidth, paletteFraction: paletteWidth / snapshot.viewport.width }, touch: { descriptorHasTouch: Boolean(config.hasTouch), maxTouchPoints: snapshot.touch.maxTouchPoints }, hudBounds: snapshot.hudBounds, controls: Object.fromEntries(requiredControls.map((control) => [control, snapshot.testBounds[control]])), errors, screenshot };
    results.push(result);
    console.log(JSON.stringify(result));
    await context.close();
  }
} finally {
  await browser.close();
}
await fs.writeFile(path.join(output, "results.json"), JSON.stringify(results, null, 2));
