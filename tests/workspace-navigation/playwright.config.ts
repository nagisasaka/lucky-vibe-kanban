import { defineConfig, devices } from "@playwright/test";

const baseURL = process.env.LVK_E2E_BASE_URL;
if (
  !baseURL ||
  new URL(baseURL).hostname !== "127.0.0.1" ||
  ["4020", "8443", "8444"].includes(new URL(baseURL).port)
) {
  throw new Error("独立した一時DBを使うloopback専用LVK_E2E_BASE_URLが必要です");
}

export default defineConfig({
  testDir: ".",
  outputDir: "../../test-results/workspace-navigation",
  workers: 1,
  timeout: 60_000,
  expect: { timeout: 15_000 },
  reporter: [["list"]],
  use: {
    ...devices["Desktop Chrome"],
    baseURL,
    screenshot: "only-on-failure",
    trace: "retain-on-failure",
  },
});
