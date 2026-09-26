import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./e2e",
  fullyParallel: false,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? "github" : "list",
  use: {
    ...devices["Desktop Chrome"],
    baseURL: "http://127.0.0.1:4174",
    headless: true,
    launchOptions: {
      args: ["--enable-webgl", "--ignore-gpu-blocklist", "--use-gl=swiftshader"],
    },
  },
  webServer: {
    command: "npm run dev -- --port 4174",
    url: "http://127.0.0.1:4174/review-harness.html",
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
});
