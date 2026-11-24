import { defineConfig } from "@playwright/test";
export default defineConfig({
  testDir: ".",
  testMatch: "*.spec.ts",
  workers: 1,
  timeout: 30000,
  outputDir: "../../test-results/run-selection",
  use: {
    baseURL: "http://127.0.0.1:4198",
    screenshot: "only-on-failure",
    launchOptions: {
      executablePath: process.env.LVK_E2E_CHROMIUM,
      args: ["--no-sandbox", "--disable-dev-shm-usage"],
    },
  },
  webServer: {
    command:
      "pnpm exec vite --config tests/run-selection/vite.config.ts --host 127.0.0.1 --port 4198 --strictPort",
    cwd: "../..",
    url: "http://127.0.0.1:4198",
    reuseExistingServer: false,
  },
});
