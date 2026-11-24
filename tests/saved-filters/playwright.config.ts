import { defineConfig } from "@playwright/test";
export default defineConfig({
  testDir: ".",
  testMatch: "*.spec.ts",
  testIgnore: "**/saved-filters/runtime/**",
  use: { baseURL: "http://127.0.0.1:4178", headless: true },
  workers: 1,
  outputDir: "../../.build/saved-filters/test-results",
  webServer: {
    command:
      "pnpm exec vite --config tests/saved-filters/vite.config.ts --host 127.0.0.1 --port 4178",
    url: "http://127.0.0.1:4178",
    reuseExistingServer: false,
  },
});
