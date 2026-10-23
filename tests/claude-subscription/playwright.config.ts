import { defineConfig, devices } from "@playwright/test";
if (
  !process.env.LVK_E2E_BASE_URL ||
  !process.env.LVK_CLAUDE_FIXTURE_TRACE ||
  !process.env.LVK_CLAUDE_TEST_ROOT
) {
  throw new Error(
    "Run with the isolated claude-subscription test server and fixture paths",
  );
}
export default defineConfig({
  testDir: ".",
  testMatch: "*.spec.ts",
  workers: 1,
  timeout: 300_000,
  expect: { timeout: 30_000 },
  outputDir: "../../test-results/claude-subscription",
  reporter: [["list"]],
  use: {
    ...devices["Desktop Chrome"],
    viewport: { width: 1280, height: 900 },
    actionTimeout: 20_000,
    navigationTimeout: 30_000,
    baseURL: process.env.LVK_E2E_BASE_URL,
    screenshot: "only-on-failure",
  },
});
