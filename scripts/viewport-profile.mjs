import path from "node:path";
const playwright = await import(process.env.PLAYWRIGHT_PATH ? path.join(process.env.PLAYWRIGHT_PATH, "index.mjs") : "playwright");
const { webkit } = playwright;
const baseUrl = process.env.TINDERBOX_TEST_URL ?? "http://127.0.0.1:4173/";
const browser = await webkit.launch({ headless: true });
const scenarios = [
  ["desktop", { width: 1180, height: 800 }, { width: 800, height: 600 }],
  ["phone-portrait", { width: 390, height: 844 }, { width: 844, height: 390 }],
];
const results = [];
try {
  for (const [viewportName, initial, rotated] of scenarios) {
    for (const mode of ["fixed", "endless"]) {
      const context = await browser.newContext({ viewport: initial, isMobile: viewportName === "phone-portrait", hasTouch: viewportName === "phone-portrait", deviceScaleFactor: 1 });
      try {
        const page = await context.newPage();
        await page.addInitScript(() => {
          const times = [];
          const loop = (time) => { times.push(time); requestAnimationFrame(loop); };
          requestAnimationFrame(loop);
          Object.defineProperty(window, "__rafTimes", { get: () => times });
        });
        await page.goto(`${baseUrl}?toolbar-test=1&profile=${mode}`, { waitUntil: "load", timeout: 60_000 });
        await page.waitForFunction(() => document.querySelector("#the_canvas_id")?.getAttribute("data-renderer")?.match(/webgl|webgpu/), null, { timeout: 60_000 });
        await page.waitForTimeout(3000);
        await page.setViewportSize(rotated);
        await page.waitForTimeout(1500);
        if (mode === "endless") {
          for (let index = 0; index < 12; index++) {
            await page.keyboard.press("ArrowRight");
            await page.waitForTimeout(25);
          }
        } else {
          const nav = await page.locator("#the_canvas_id").evaluate((canvas) => canvas.getAttribute("data-toolbar-navigate"));
          if (nav) {
            const [x0, y0, x1, y1] = nav.split(",").map(Number);
            await page.mouse.click((x0 + x1) / 2, (y0 + y1) / 2);
          }
          const bounds = await page.locator("#the_canvas_id").boundingBox();
          if (bounds) {
            await page.mouse.move(bounds.x + bounds.width * 0.6, bounds.y + bounds.height * 0.5);
            await page.mouse.down();
            await page.mouse.move(bounds.x + bounds.width * 0.3, bounds.y + bounds.height * 0.5, { steps: 12 });
            await page.mouse.up();
          }
        }
        await page.waitForFunction(() => document.querySelector("#the_canvas_id")?.getAttribute("data-frame-ms") !== null, null, { timeout: 10_000 });
        await page.waitForFunction(
          () => document.querySelector("#the_canvas_id")?.hasAttribute("data-profile-report"),
          null,
          { timeout: 30_000 },
        );
        const metrics = await page.evaluate(() => {
          const times = window.__rafTimes ?? [];
          const frameIntervals = times.slice(1).map((time, index) => time - times[index]).filter(Number.isFinite);
          const sorted = [...frameIntervals].sort((a, b) => a - b);
          const percentile = (p) => sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * p))] ?? 0;
          return {
            rAF_samples: frameIntervals.length,
            raf_avg_ms: frameIntervals.reduce((sum, value) => sum + value, 0) / Math.max(frameIntervals.length, 1),
            raf_p99_ms: percentile(0.99),
            raf_worst_ms: sorted.at(-1) ?? 0,
            raf_over_33ms: frameIntervals.filter((value) => value > 33.3).length,
            app_frame_ms: Number(document.querySelector("#the_canvas_id")?.getAttribute("data-frame-ms")),
            profile_report: document.querySelector("#the_canvas_id")?.getAttribute("data-profile-report"),
            scrollY,
            viewport: { innerWidth, innerHeight },
          };
        });
        if (!metrics.profile_report?.includes(`mode=${mode};`) || !metrics.profile_report.includes("frames=900;") || metrics.rAF_samples < 850) {
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
