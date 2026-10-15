import { chromium } from "playwright";
import fs from "node:fs/promises";

const output = "tmp/mobile-setup-toolbar-smoke";
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
    await page.waitForTimeout(500);
    const before = await page.screenshot({ path: `${output}/${name}-setup.png` });

    // Scroll setup using touch input, then reload so the action check starts at the top.
    const session = await context.newCDPSession(page);
    const x = Math.min(width - 24, 300);
    const y = Math.floor(height * 0.46 + 12);
    for (let swipe = 0; swipe < 12; swipe++) {
      await session.send("Input.dispatchTouchEvent", {
        type: "touchStart",
        touchPoints: [{ x, y }],
      });
      for (let step = 1; step <= 4; step++) {
        await session.send("Input.dispatchTouchEvent", {
          type: "touchMove",
          touchPoints: [{ x, y: y - step * 35 }],
        });
      }
      await session.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
      await page.waitForTimeout(100);
    }
    const scrolled = await page.screenshot({ path: `${output}/${name}-setup-scrolled.png` });
    const scrollChanged = before.reduce((n, byte, i) => n + (byte !== scrolled[i]), 0);
    if (scrollChanged < 1_000) throw new Error(`${name}: setup did not visibly scroll from touch input`);
    await session.detach();

    await page.reload({ waitUntil: "load", timeout: 60_000 });
    await page.waitForFunction(
      () => document.querySelector("#the_canvas_id")?.getAttribute("data-renderer") === "webgl",
      null,
      { timeout: 60_000 },
    );
    await page.waitForTimeout(300);
    const cancel = { portrait: [145, 204], landscape: [145, 197], narrow: [145, 197] }[name];
    await page.touchscreen.tap(...cancel);
    await page.waitForTimeout(300);
    const toolbar = await page.screenshot({ path: `${output}/${name}-toolbar.png` });
    const cancelChanged = before.reduce((n, byte, i) => n + (byte !== toolbar[i]), 0);
    if (cancelChanged < 1_000) throw new Error(`${name}: Cancel touch did not visibly close setup`);

    await page.touchscreen.tap(30, 24);
    await page.waitForTimeout(450);
    const playing = await page.screenshot({ path: `${output}/${name}-playing.png` });
    const playChanged = toolbar.reduce((n, byte, i) => n + (byte !== playing[i]), 0);
    if (playChanged < 1_000) throw new Error(`${name}: primary Play touch caused no visible state change`);

    const westY = name === "landscape" ? 116 : 256;
    await page.touchscreen.tap(70, westY);
    await page.waitForTimeout(200);
    const west = await page.screenshot({ path: `${output}/${name}-wind-west.png` });
    const windChanged = playing.reduce((n, byte, i) => n + (byte !== west[i]), 0);
    if (windChanged < 1_000) throw new Error(`${name}: Wind West touch caused no visible state change`);
    if (errors.length) throw new Error(`${name}: ${errors.join("\n")}`);

    results.push({
      name,
      width,
      height,
      scrollChangedPngBytes: scrollChanged,
      cancelChangedPngBytes: cancelChanged,
      playChangedPngBytes: playChanged,
      windChangedPngBytes: windChanged,
      renderer: await page.locator("#the_canvas_id").getAttribute("data-renderer"),
      errors,
    });
    await context.close();

    const playContext = await browser.newContext({
      viewport: { width, height },
      deviceScaleFactor: 1,
      isMobile: true,
      hasTouch: true,
    });
    const playPage = await playContext.newPage();
    await playPage.goto(baseUrl, { waitUntil: "load", timeout: 60_000 });
    await playPage.waitForFunction(
      () => document.querySelector("#the_canvas_id")?.getAttribute("data-renderer") === "webgl",
      null,
      { timeout: 60_000 },
    );
    await playPage.waitForTimeout(1_000);
    const setupStart = await playPage.screenshot({ path: `${output}/${name}-play-start.png` });
    const playSetup = { portrait: [38, 205], landscape: [38, 197], narrow: [38, 197] }[name];
    await playPage.touchscreen.tap(...playSetup);
    await playPage.waitForTimeout(500);
    const afterSetupPlay = await playPage.screenshot({ path: `${output}/${name}-play-setup.png` });
    const setupPlayChanged = setupStart.reduce((n, byte, i) => n + (byte !== afterSetupPlay[i]), 0);
    await playPage.touchscreen.tap(30, 24);
    await playPage.waitForTimeout(450);
    const playProbe = await playPage.screenshot({ path: `${output}/${name}-play-probe.png` });
    const playProbeChanged = afterSetupPlay.reduce((n, byte, i) => n + (byte !== playProbe[i]), 0);
    if (setupPlayChanged < 1_000 || playProbeChanged < 1_000) {
      throw new Error(`${name}: setup Play did not visibly close the setup and enable toolbar interaction`);
    }
    results[results.length - 1].setupPlayChangedPngBytes = setupPlayChanged;
    results[results.length - 1].playProbeChangedPngBytes = playProbeChanged;
    await playContext.close();
  }
} finally {
  await browser.close();
}
await fs.writeFile(`${output}/results.json`, JSON.stringify(results, null, 2));
console.log(JSON.stringify(results, null, 2));
