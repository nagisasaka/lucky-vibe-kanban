import { defineConfig } from "@playwright/test";
export default defineConfig({
  testDir: ".",
  testMatch: "*.spec.ts",
  workers: 1,
  timeout: 60000,
  outputDir: "../../test-results/settings-redesign",
  use: {
    actionTimeout: 10000,
    baseURL: "http://127.0.0.1:4196",
    launchOptions: {
      executablePath: process.env.LVK_E2E_CHROMIUM,
      args: ["--no-sandbox", "--disable-dev-shm-usage"],
    },
  },
  webServer: {
    command:
      "pnpm exec vite --config tests/settings-redesign/vite.config.ts --host 127.0.0.1 --port 4196 --strictPort",
    cwd: "../..",
    url: "http://127.0.0.1:4196",
    reuseExistingServer: false,
    timeout: 120000,
  },
});
