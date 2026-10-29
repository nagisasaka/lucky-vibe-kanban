import { defineConfig, devices } from "@playwright/test";
if (
  !process.env.LVK_E2E_BASE_URL ||
  !process.env.LVK_REVIEW_FIXTURE_TRACE ||
  !process.env.LVK_REVIEW_TEST_ROOT
) {
  throw new Error("Run with the isolated REVIEW test server and fixture paths");
}
export default defineConfig({
  testDir: ".",
  testMatch: "*.spec.ts",
  workers: 1,
  timeout: 120_000,
  expect: { timeout: 30_000 },
  outputDir: "../../test-results/review-mode",
  reporter: [["list"]],
  use: {
    ...devices["Desktop Chrome"],
    viewport: { width: 1440, height: 900 },
    baseURL: process.env.LVK_E2E_BASE_URL,
    screenshot: "only-on-failure",
    actionTimeout: 30_000,
  },
});
