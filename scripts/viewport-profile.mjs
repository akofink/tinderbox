import path from "node:path";
const playwright = await import(process.env.PLAYWRIGHT_PATH ? path.join(process.env.PLAYWRIGHT_PATH, "index.mjs") : "playwright");
const { chromium, webkit } = playwright;
const browserName = process.env.TINDERBOX_BROWSER ?? "webkit";
const browserType = browserName === "chromium" ? chromium : webkit;
const baseUrl = process.env.TINDERBOX_TEST_URL ?? "http://127.0.0.1:4173/";
const browser = await browserType.launch({
  headless: true,
  ...(browserName === "chromium" ? { args: ["--enable-webgl", "--ignore-gpu-blocklist", "--use-gl=angle", "--use-angle=swiftshader", "--enable-unsafe-swiftshader"] } : {}),
});
const scenarios = [
  ["desktop", { width: 1180, height: 800 }, { width: 800, height: 600 }],
  ["phone-portrait-to-landscape", { width: 390, height: 844 }, { width: 844, height: 390 }],
  ["phone-landscape-to-portrait", { width: 844, height: 390 }, { width: 390, height: 844 }],
];
const results = [];
try {
  for (const [viewportName, initial, rotated] of scenarios) {
    for (const mode of ["fixed", "endless"]) {
      const phone = viewportName.startsWith("phone-");
      const context = await browser.newContext({ viewport: initial, isMobile: phone, hasTouch: phone, deviceScaleFactor: 1 });
      try {
        const page = await context.newPage();
        await page.addInitScript(() => {
          const times = [];
          const marks = {};
          const loop = (time) => { times.push(time); requestAnimationFrame(loop); };
          requestAnimationFrame(loop);
          Object.defineProperties(window, {
            __rafTimes: { get: () => times },
            __profileMarks: { get: () => marks },
            __profileMark: { value: (name) => { marks[name] = performance.now(); } },
          });
        });
        await page.goto(`${baseUrl}?toolbar-test=1&profile=${mode}`, { waitUntil: "load", timeout: 60_000 });
        await page.waitForFunction(() => document.querySelector("#the_canvas_id")?.getAttribute("data-renderer")?.match(/webgl|webgpu/), null, { timeout: 60_000 });
        await page.waitForTimeout(3000);
        const togglePlay = async (expectedRunning) => {
          const bounds = await page.locator("#the_canvas_id").getAttribute("data-toolbar-play-pause");
          const [, y0, , y1] = bounds.split(",").map(Number);
          const canvasTop = await page.locator("#the_canvas_id").evaluate((canvas) => canvas.getBoundingClientRect().top);
          await page.mouse.click(32, canvasTop + (y0 + y1) / 2);
          await page.waitForFunction((expected) => {
            const value = document.querySelector("#the_canvas_id")?.getAttribute("data-toolbar-play-pause")?.split(",")[8];
            return value === String(expected);
          }, expectedRunning);
        };
        if (mode === "endless") await togglePlay(false);
        const beforeRotation = await page.locator("#the_canvas_id").evaluate((canvas) => ({
          worldSize: canvas.getAttribute("data-world-size"),
          dropped: canvas.getAttribute("data-dropped-cells"),
          globalCameraCenterX: canvas.getAttribute("data-global-camera-center-x"),
        }));
        await page.evaluate(() => window.__profileMark("resizeStart"));
        await page.setViewportSize(rotated);
        await page.waitForTimeout(750);
        const afterRotation = await page.locator("#the_canvas_id").evaluate((canvas) => ({
          worldSize: canvas.getAttribute("data-world-size"),
          dropped: canvas.getAttribute("data-dropped-cells"),
          globalCameraCenterX: canvas.getAttribute("data-global-camera-center-x"),
          toolbar: canvas.getAttribute("data-layout-toolbar")?.split(",").map(Number),
          world: canvas.getAttribute("data-layout-world")?.split(",").map(Number),
        }));
        const [beforeWidth, beforeHeight] = beforeRotation.worldSize?.split("x").map(Number) ?? [];
        const [afterWidth, afterHeight] = afterRotation.worldSize?.split("x").map(Number) ?? [];
        if (!beforeWidth || !afterWidth || beforeHeight !== afterHeight ||
            (mode === "fixed" && afterRotation.worldSize !== beforeRotation.worldSize) ||
            (mode === "endless" && afterWidth % 128 !== 0)) {
          throw new Error(`${viewportName}/${mode}: world dimensions/span incoherent after rotation ${JSON.stringify({ beforeRotation, afterRotation })}`);
        }
        if (beforeRotation.dropped !== "0" || afterRotation.dropped !== "0") {
          throw new Error(`${viewportName}/${mode}: nonzero dropped-cell count around rotation ${JSON.stringify({ beforeRotation, afterRotation })}`);
        }
        if (mode === "endless" && Math.abs(Number(beforeRotation.globalCameraCenterX) - Number(afterRotation.globalCameraCenterX)) > 0.05) {
          throw new Error(`${viewportName}: global world coordinate at viewport center shifted during rotation ${JSON.stringify({ beforeRotation, afterRotation })}`);
        }
        if (mode === "endless") await togglePlay(true);
        if (!afterRotation.toolbar || !afterRotation.world || afterRotation.toolbar[3] > afterRotation.world[1] + 1) {
          throw new Error(`${viewportName}/${mode}: toolbar clips/overlaps world ${JSON.stringify(afterRotation)}`);
        }
        await page.evaluate(() => window.__profileMark("resizeEnd"));
        await page.evaluate(() => window.__profileMark("navigationStart"));
        await page.locator("#the_canvas_id").evaluate((canvas) => canvas.focus());
        for (let index = 0; index < 12; index++) {
          await page.keyboard.press("ArrowRight");
          await page.waitForTimeout(25);
        }
        await page.waitForTimeout(500);
        await page.evaluate(() => window.__profileMark("navigationEnd"));
        await page.waitForFunction(() => document.querySelector("#the_canvas_id")?.getAttribute("data-frame-ms") !== null, null, { timeout: 10_000 });
        await page.waitForFunction(
          () => document.querySelector("#the_canvas_id")?.hasAttribute("data-profile-report"),
          null,
          { timeout: 30_000 },
        );
        const metrics = await page.evaluate(() => {
          const times = window.__rafTimes ?? [];
          const marks = window.__profileMarks ?? {};
          const frames = times.slice(1).map((end, index) => ({ start: times[index], end, ms: end - times[index] })).filter((frame) => Number.isFinite(frame.ms));
          const stats = (start, end) => {
            start = Number.isFinite(start) ? start : (frames[0]?.start ?? 0);
            end = Number.isFinite(end) ? end : (frames.at(-1)?.end ?? 0);
            const samples = frames.filter((frame) => frame.start >= start && frame.end <= end).map((frame) => frame.ms);
            const sorted = [...samples].sort((a, b) => a - b);
            return {
              start_ms: Number(start.toFixed(1)), end_ms: Number(end.toFixed(1)), samples: samples.length,
              avg_ms: samples.reduce((sum, value) => sum + value, 0) / Math.max(samples.length, 1),
              p99_ms: sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * 0.99))] ?? 0,
              worst_ms: sorted.at(-1) ?? 0,
              over_33ms: samples.filter((value) => value > 33.3).length,
            };
          };
          const resizeStart = marks.resizeStart;
          const resizeEnd = marks.resizeEnd;
          const navStart = marks.navigationStart;
          const navEnd = marks.navigationEnd;
          const canvas = document.querySelector("#the_canvas_id");
          return {
            viewport: { innerWidth, innerHeight }, scrollY,
            all: stats(frames[0]?.start ?? 0, frames.at(-1)?.end ?? 0),
            phases: {
              baseline: stats(resizeStart - 1500, resizeStart),
              resize: stats(resizeStart, resizeEnd),
              navigation: stats(navStart, navEnd),
            },
            profile_report: canvas?.getAttribute("data-profile-report"),
          };
        });
        if (!metrics.profile_report?.includes(`mode=${mode};`) || !metrics.profile_report.includes("frames=900;") ||
            metrics.all.samples < 850 || Object.values(metrics.phases).some((phase) => phase.samples < 10)) {
          throw new Error(`${viewportName}/${mode}: incomplete profile: ${JSON.stringify(metrics)}`);
        }
        const result = { viewportName, mode, initial, rotated, metrics };
        results.push(result);
        console.log(JSON.stringify(result));
      } finally {
        await context.close();
      }
    }
  }
} finally {
  await browser.close();
}
