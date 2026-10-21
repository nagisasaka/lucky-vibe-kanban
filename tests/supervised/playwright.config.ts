import { defineConfig, devices } from "@playwright/test";
if (
  !process.env.LVK_E2E_BASE_URL ||
  !process.env.LVK_APPROVAL_FIXTURE_TRACE ||
  !process.env.LVK_SUPERVISED_TEST_ROOT
) {
  throw new Error(
    "Run with the isolated supervised test server and fixture paths",
  );
}
export default defineConfig({
  testDir: ".",
  testMatch: "*.spec.ts",
  workers: 1,
  timeout: 300_000,
  expect: { timeout: 30_000 },
  outputDir: "../../test-results/supervised",
  reporter: [["list"]],
  use: {
    ...devices["Desktop Chrome"],
    viewport: { width: 600, height: 900 },
    baseURL: process.env.LVK_E2E_BASE_URL,
    screenshot: "only-on-failure",
  },
});
