import { defineConfig } from "@playwright/test";
export default defineConfig({
  testDir: ".",
  testMatch: "*.spec.ts",
  workers: 1,
  maxFailures: 1,
  timeout: 120_000,
  outputDir: "../../test-results/terminal-latency",
  use: {
    baseURL: process.env.LVK_E2E_BASE_URL,
    headless: true,
    actionTimeout: 15_000,
    launchOptions: { executablePath: process.env.LVK_E2E_CHROMIUM },
  },
});
