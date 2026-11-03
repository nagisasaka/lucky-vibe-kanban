import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { strict as assert } from "node:assert";
import test from "node:test";

// Preserve the current pre-redesign cards, including subsequent i18n fixes.
// Compare whole card bodies, including handlers/defaults, rather than just labels.
const file =
  "packages/web-core/src/shared/dialogs/settings/settings/GeneralSettingsSection.tsx";
const baseline = execFileSync(
  "git",
  ["show", `43774ca91388b9b4b0f2b78f23e3099f1eea6825:${file}`],
  { encoding: "utf8" },
);
const current = readFileSync(file, "utf8");
const cards = (source) =>
  [...source.matchAll(/<SettingsCard\b[\s\S]*?<\/SettingsCard>/g)].map(
    (match) =>
      match[0]
        .replace(
          /<p className="text-xs text-low">\s*\{t\('settings\.scope\.[^']+'\)\}\s*<\/p>/g,
          "",
        )
        .replace(/\s+/g, "")
        .replace(/,([}\]])/g, "$1"),
  );

test("all twelve General cards retain controls, defaults and handlers after presentation redesign", () => {
  assert.equal(cards(baseline).length, 12);
  assert.deepEqual(cards(current), cards(baseline));
});
