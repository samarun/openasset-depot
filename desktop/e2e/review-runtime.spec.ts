import { expect, test } from "@playwright/test";

for (const [fixture, animated] of [
  ["animated-cube.glb", true],
  ["simple-triangle.gltf", false],
  ["animated-cube.fbx", true],
] as const) {
  test(`${fixture} ${animated ? "decodes, animates, and" : "decodes and"} paints non-background pixels`, async ({ page }) => {
    const browserErrors: string[] = [];
    page.on("console", (message) => {
      if (message.type() === "error") browserErrors.push(message.text());
    });
    page.on("pageerror", (error) => browserErrors.push(error.message));

    await page.goto(`/review-harness.html?fixture=/e2e/fixtures/${fixture}&fps=24000%2F1001`);
    const canvas = page.locator(".three-review-viewport canvas");
    await expect(canvas).toBeVisible();
    await expect(page.locator(".review-media-error")).toHaveCount(0);
    if (animated) {
      await expect.poll(async () => Number(await page.locator("body").getAttribute("data-timecode-ms")))
        .toBeGreaterThan(100);
    }

    const paintedPixels = await canvas.evaluate((element) => {
      const source = element as HTMLCanvasElement;
      const probe = document.createElement("canvas");
      probe.width = source.width;
      probe.height = source.height;
      const context = probe.getContext("2d", { willReadFrequently: true });
      if (!context) return 0;
      context.drawImage(source, 0, 0);
      const pixels = context.getImageData(0, 0, probe.width, probe.height).data;
      let varied = 0;
      for (let index = 0; index < pixels.length; index += 16) {
        const red = pixels[index];
        const green = pixels[index + 1];
        const blue = pixels[index + 2];
        const alpha = pixels[index + 3];
        const distanceFromBackground = Math.abs(red - 21) + Math.abs(green - 25) + Math.abs(blue - 28);
        if (alpha > 200 && distanceFromBackground > 24) varied += 1;
      }
      return varied;
    });
    expect(paintedPixels).toBeGreaterThan(100);
    expect(browserErrors).toEqual([]);
  });
}
