import { defineConfig } from '@playwright/test';

// All API responses in these UI tests are fixtures. No Auth0 tenant or LVK DB is used.
export default defineConfig({
  testDir: '.',
  testMatch: '*.spec.ts',
  outputDir: '../../test-results/auth0-setup',
  workers: 1,
  timeout: 120_000,
  expect: { timeout: 30_000 },
  use: {
    baseURL: 'http://127.0.0.1:4189',
    launchOptions: {
      executablePath: process.env.LVK_E2E_CHROMIUM,
      args: ['--no-sandbox', '--disable-dev-shm-usage'],
    },
    trace: 'off',
  },
  webServer: {
    command: 'pnpm --dir packages/local-web exec vite --host 127.0.0.1 --port 4189 --strictPort',
    cwd: '../..',
    url: 'http://127.0.0.1:4189',
    reuseExistingServer: false,
    timeout: 120_000,
  },
});
