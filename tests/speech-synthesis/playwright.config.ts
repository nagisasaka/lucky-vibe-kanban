import { defineConfig } from "@playwright/test";
export default defineConfig({
  testDir: ".",
  testMatch: "*.spec.ts",
  outputDir: "../../test-results/speech-synthesis",
  workers: 1,
  timeout: 60_000,
  expect: { timeout: 20_000 },
  use: {
    baseURL: "http://127.0.0.1:4198",
    launchOptions: {
      executablePath: process.env.LVK_E2E_CHROMIUM,
      args: ["--no-sandbox", "--disable-dev-shm-usage"],
    },
    trace: "off",
  },
  webServer: {
    command:
      "pnpm exec vite --config ../../tests/speech-synthesis/vite.config.ts --host 127.0.0.1 --port 4198 --strictPort",
    cwd: "../../packages/local-web",
    url: "http://127.0.0.1:4198",
    reuseExistingServer: false,
    timeout: 120_000,
  },
});
