import path from "node:path";
const playwright = await import(process.env.PLAYWRIGHT_PATH ? path.join(process.env.PLAYWRIGHT_PATH, "index.mjs") : "playwright");
const { webkit } = playwright;
const baseUrl = process.env.TINDERBOX_TEST_URL ?? "http://127.0.0.1:4173/";
const browser = await webkit.launch({ headless: true });
try {
  const context = await browser.newContext({ viewport: { width: 390, height: 844 }, deviceScaleFactor: 1, isMobile: true });
  try {
    const page = await context.newPage();
    await page.goto(baseUrl, { waitUntil: "load", timeout: 60_000 });
    await page.waitForFunction(
      () => ["webgl", "webgpu"].includes(document.querySelector("#the_canvas_id")?.getAttribute("data-renderer")),
      null,
      { timeout: 60_000 },
    );
    for (const viewport of [{ width: 844, height: 390 }, { width: 390, height: 844 }]) {
      await page.setViewportSize(viewport);
      await page.waitForTimeout(250);
      const result = await page.evaluate(() => {
        const canvas = document.querySelector("#the_canvas_id");
        const bounds = canvas.getBoundingClientRect();
        return { scrollY, innerWidth, innerHeight, canvas: { x: bounds.x, y: bounds.y, width: bounds.width, height: bounds.height } };
      });
      if (result.scrollY !== 0) throw new Error(`page scrolled after viewport transition: ${JSON.stringify(result)}`);
      const tolerance = 1;
      if (Math.abs(result.canvas.x) > tolerance || Math.abs(result.canvas.y) > tolerance ||
          Math.abs(result.canvas.width - viewport.width) > tolerance ||
          Math.abs(result.canvas.height - viewport.height) > tolerance) {
        throw new Error(`canvas does not fit transitioned viewport: ${JSON.stringify({ viewport, result })}`);
      }
      console.log(JSON.stringify({ viewport, result }));
    }
  } finally {
    await context.close();
  }
} finally {
  await browser.close();
}
