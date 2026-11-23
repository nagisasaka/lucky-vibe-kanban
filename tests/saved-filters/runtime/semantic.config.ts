import { defineConfig } from "@playwright/test";

// Separate from the machine suite: never discover or launch providers by default.
export default defineConfig({
  testDir: ".",
  testMatch: "semantic.spec.ts",
  workers: 1,
  retries: 0,
  timeout: 600000,
  expect: { timeout: 45000 },
  use: {
    headless: true,
    viewport: { width: 1440, height: 1000 },
    actionTimeout: 10000,
    trace: "off",
    screenshot: "only-on-failure",
  },
  outputDir: "../../../.build/saved-filters/semantic/results",
});
