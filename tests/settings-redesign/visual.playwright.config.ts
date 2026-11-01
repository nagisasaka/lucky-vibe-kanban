import base from "./playwright.config";
import { defineConfig } from "@playwright/test";
export default defineConfig({
  ...base,
  testMatch: "visual.spec.ts",
  outputDir: "../../.build/settings-visual-review/test-results",
});
