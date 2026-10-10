import { defineConfig, devices } from "@playwright/test";

// The chat fixture mocks its APIs and must never use a running LVK deployment.
const port = 4176;
process.env.LVK_E2E_SOURCE_ROOT = process.cwd();

export default defineConfig({
  testDir: "./specs",
  testMatch: "project-chat.spec.ts",
  outputDir: "../../test-results/project-chat",
  workers: 1,
  timeout: 120_000,
  expect: { timeout: 30_000 },
  reporter: "list",
  use: {
    ...devices["Desktop Chrome"],
    baseURL: `http://127.0.0.1:${port}`,
  },
  webServer: {
    cwd: process.cwd(),
    command: `pnpm exec vite --host 127.0.0.1 --port ${port} --strictPort --config tests/server/project-chat.vite.config.ts`,
    url: `http://127.0.0.1:${port}`,
    reuseExistingServer: false,
    timeout: 120_000,
  },
});
