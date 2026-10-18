import { defineConfig } from "@playwright/test";

// All API responses in these UI tests are fixtures. No live LVK DB is used.
export default defineConfig({
  testDir: ".",
  testMatch: "*.spec.ts",
  outputDir: "../../test-results/integration-skip",
  workers: 1,
  timeout: 120_000,
  expect: { timeout: 30_000 },
  use: {
    baseURL: "http://127.0.0.1:4189",
    launchOptions: {
      executablePath: process.env.LVK_E2E_CHROMIUM,
      args: ["--no-sandbox", "--disable-dev-shm-usage"],
    },
    trace: "off",
  },
  webServer: {
    command:
      "pnpm exec vite --config ../../tests/integration-skip/vite.config.ts --host 127.0.0.1 --port 4189 --strictPort",
    cwd: "../../packages/local-web",
    url: "http://127.0.0.1:4189",
    reuseExistingServer: false,
    timeout: 120_000,
  },
});
