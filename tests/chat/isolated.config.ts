import { resolve } from "node:path";
import { defineConfig } from "@playwright/test";
const evidence =
  process.env.LVK_E2E_EVIDENCE ??
  resolve(__dirname, "../../test-results/local21");
const baseURL = process.env.LVK_E2E_BASE_URL;
if (!baseURL || new URL(baseURL).hostname !== "127.0.0.1")
  throw new Error("Dedicated loopback LVK_E2E_BASE_URL required");
export default defineConfig({
  testDir: ".",
  testMatch: "isolated.spec.ts",
  workers: 1,
  timeout: 240_000,
  expect: { timeout: 20_000 },
  outputDir: `${evidence}/browser`,
  reporter: [
    ["list"],
    [
      "json",
      {
        outputFile: `${evidence}/e2e.json`,
      },
    ],
  ],
  use: {
    baseURL,
    actionTimeout: 20_000,
    navigationTimeout: 30_000,
    viewport: { width: 1440, height: 1000 },
    launchOptions: { args: ["--no-sandbox", "--disable-dev-shm-usage"] },
    screenshot: "only-on-failure",
    trace: "retain-on-failure",
  },
});
