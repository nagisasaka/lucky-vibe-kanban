import { defineConfig } from "@playwright/test";
export default defineConfig({
  testDir: ".",
  testMatch: "acceptance.spec.ts",
  workers: 1,
  timeout: 300000,
  expect: { timeout: 45000 },
  use: {
    headless: true,
    actionTimeout: 10000,
    screenshot: "only-on-failure",
    viewport: { width: 1440, height: 1000 },
    trace: "off",
  },
  outputDir: "../../../.build/saved-filters/runtime/results",
});
