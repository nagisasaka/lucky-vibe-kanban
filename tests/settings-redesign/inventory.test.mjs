import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { strict as assert } from "node:assert";
import test from "node:test";

// Preserve the pre-regrouping cards including LOCAL-285 notification events.
// Compare whole card bodies, including handlers/defaults, rather than just labels.
const file =
  "packages/web-core/src/shared/dialogs/settings/settings/GeneralSettingsSection.tsx";
const baseline = execFileSync(
  "git",
  ["show", `6823bcc29ab0e2113de0b9cac85fc355b366ba87:${file}`],
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

test("all twelve original General cards retain controls, defaults and handlers after regrouping", () => {
  assert.equal(cards(baseline).length, 12);
  assert.deepEqual(cards(current), cards(baseline));
});
