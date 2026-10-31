import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { strict as assert } from "node:assert";
import test from "node:test";

// One-off prototype inventory against the agreed, immutable starting point.
// Compare whole card bodies, including handlers/defaults, rather than just labels.
const file =
  "packages/web-core/src/shared/dialogs/settings/settings/GeneralSettingsSection.tsx";
const baseline = execFileSync(
  "git",
  ["show", `89c1a09517f293f9409d8ba6f0d4279868cd36e9:${file}`],
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
