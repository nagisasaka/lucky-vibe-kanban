import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

function inspect(
  source,
  en,
  ja,
  allowlist = [],
  schemas = {},
  otherLocales = {},
  checker = "scripts/check-i18n.mjs",
) {
  ja ??= en;
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "lvk-i18n-check-"));
  try {
    for (const pkg of ["web-core", "local-web", "ui"])
      fs.mkdirSync(path.join(root, "packages", pkg, "src"), {
        recursive: true,
      });
    fs.writeFileSync(path.join(root, "packages/ui/src/Fixture.tsx"), source);
    for (const [language, namespaces] of Object.entries({
      ...otherLocales,
      en,
      ja,
    })) {
      const directory = path.join(
        root,
        "packages/web-core/src/i18n/locales",
        language,
      );
      fs.mkdirSync(directory, { recursive: true });
      for (const [namespace, value] of Object.entries(namespaces))
        fs.writeFileSync(
          path.join(directory, `${namespace}.json`),
          typeof value === "string" ? value : JSON.stringify(value),
        );
    }
    fs.mkdirSync(path.join(root, "scripts"));
    fs.writeFileSync(
      path.join(root, "scripts/i18n-allowlist.json"),
      JSON.stringify(allowlist),
    );
    fs.mkdirSync(path.join(root, "shared/schemas"), { recursive: true });
    for (const [name, schema] of Object.entries(schemas))
      fs.writeFileSync(
        path.join(root, "shared/schemas", `${name}.json`),
        JSON.stringify(schema),
      );
    const result = spawnSync(
      process.execPath,
      [checker, "--root", root, "--json"],
      { encoding: "utf8" },
    );
    assert.equal(result.signal, null);
    assert.equal(result.stderr, "");
    return { status: result.status, ...JSON.parse(result.stdout) };
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

test("resolves namespaces per component, aliased hooks, explicit namespaces and passed translators", () => {
  const report = inspect(
    `
    function action(t) { return t('action'); }
    function Component() {
      const { t } = useTranslation('tasks');
      const { t: commonT } = useTranslation('common');
      return [action(t), commonT('ok'), t('common:ok'), t('ok', { ns: 'common' })];
    }
  `,
    { common: { ok: "OK" }, tasks: { action: "Action" } },
  );
  assert.equal(report.status, 0);
  assert.equal(report.counts.calls, 4);
});

test("accepts Japanese plural rules and rejects missing, empty, duplicate and interpolation errors", () => {
  const report = inspect(
    `function C() { const { t } = useTranslation(); return t('absent'); }`,
    {
      common: {
        item_one: "{{count}} item",
        item_other: "{{count}} items",
        missing: "Missing",
        blank: "Blank",
        bad: "{{name}}",
      },
    },
    {
      common:
        '{"item":"{{count}}件","blank":"","bad":"{{id}}","duplicate":"a","duplicate":"b"}',
    },
  );
  assert.equal(report.status, 1);
  assert.equal(report.summary["missing-ja"], 1);
  assert.equal(report.summary["empty-or-invalid"], 1);
  assert.equal(report.summary.duplicate, 1);
  assert.equal(report.summary.interpolation, 1);
  assert.equal(report.summary["missing-en"], 1);
});

test("checks direct JSX text, referenced keys and unresolved dynamic keys", () => {
  const report = inspect(
    `function C() {
    const { t } = useTranslation();
    return <button>Save {t('missing', { defaultValue: 'Fallback' })}{t(key)}</button>;
  }`,
    { common: {} },
  );
  assert.equal(report.status, 1);
  assert.equal(report.summary["hardcode-markup"], 1);
  assert.equal(report.summary["dynamic-key"], 1);
  assert.equal(report.summary["missing-en"], 1);
  assert.equal(report.counts.defaultValues, 1);
});

test("allows only exact documented tokens and rejects broad or stale exceptions", () => {
  const token = {
    kind: "hardcode-markup",
    file: "packages/ui/src/Fixture.tsx",
    value: "LVK",
    reason: "Product name",
  };
  const allowed = inspect(
    "const view = <span>LVK</span>;",
    { common: {} },
    undefined,
    [token],
  );
  assert.equal(allowed.status, 0);
  assert.equal(allowed.allowlisted.length, 1);
  const stale = inspect(
    "const view = <span>Save</span>;",
    { common: {} },
    undefined,
    [token],
  );
  assert.equal(stale.summary["stale-allowlist"], 1);
  assert.equal(stale.summary["hardcode-markup"], 1);
  const broad = inspect(
    "const view = <span>Save</span>;",
    { common: {} },
    undefined,
    [{ ...token, file: "packages/ui/**" }],
  );
  assert.equal(broad.summary["invalid-allowlist"], 1);
  assert.equal(broad.summary["hardcode-markup"], 1);
});

test("requires generated schema annotation keys, exact English source and Japanese coverage", () => {
  const schema = {
    properties: {
      model: {
        title: "Model",
        "x-lvk-i18n-title": "settings:executorSchemas.codex.model.title",
      },
    },
  };
  const en = {
    settings: { executorSchemas: { codex: { model: { title: "Model" } } } },
  };
  const ja = {
    settings: { executorSchemas: { codex: { model: { title: "モデル" } } } },
  };
  assert.equal(inspect("", en, ja, [], { codex: schema }).status, 0);
  const dynamicSource = "function label(t, key: string) { return t(key); }";
  const annotationCall = [
    {
      kind: "dynamic-key",
      file: "packages/ui/src/Fixture.tsx",
      value: "key",
      reason: "Generated annotation keys are separately checked below",
    },
  ];
  assert.equal(
    inspect(dynamicSource, en, ja, annotationCall, { codex: schema }).status,
    0,
  );
  const drift = structuredClone(schema);
  drift.properties.model.title = "Changed";
  assert(
    inspect(dynamicSource, en, ja, annotationCall, {
      codex: drift,
    }).findings.some((f) => f.kind === "schema-source-drift"),
  );
  assert(
    inspect("", en, { settings: {} }, [], { codex: schema }).findings.some(
      (f) => f.kind === "schema-missing-ja",
    ),
  );
  delete drift.properties.model["x-lvk-i18n-title"];
  assert(
    inspect("", en, ja, [], { codex: drift }).findings.some(
      (f) => f.kind === "schema-i18n-key",
    ),
  );
});

test("requires complete enum label references tied to canonical values", () => {
  const en = {
    settings: {
      executorSchemas: {
        codex: { model: { title: "Model", options: { auto: "Automatic" } } },
      },
    },
  };
  const ja = {
    settings: {
      executorSchemas: {
        codex: { model: { title: "モデル", options: { auto: "自動" } } },
      },
    },
  };
  const schema = {
    properties: {
      model: {
        title: "Model",
        enum: ["auto"],
        "x-lvk-i18n-title": "settings:executorSchemas.codex.model.title",
        "x-lvk-i18n-enum": [
          {
            key: "settings:executorSchemas.codex.model.options.auto",
            source: "Automatic",
          },
        ],
      },
    },
  };
  assert.equal(inspect("", en, ja, [], { codex: schema }).status, 0);
  schema.properties.model.enum.push(null);
  assert(
    inspect("", en, ja, [], { codex: schema }).findings.some(
      (f) => f.kind === "schema-enum-labels",
    ),
  );
  schema.properties.model.enum.pop();
  schema.properties.model["x-lvk-i18n-enum"][0].key += "_wrong";
  assert(
    inspect("", en, ja, [], { codex: schema }).findings.some(
      (f) => f.kind === "schema-i18n-key",
    ),
  );
});

test("namespace arrays bind only their first namespace unless fallback mode is explicit", () => {
  const dictionaries = {
    tasks: { own: "Owned" },
    common: { badge: "Current" },
  };
  const report = inspect(
    `
    function C() {
      const { t } = useTranslation(['tasks', 'common']);
      const { t: otherT } = useTranslation(['common', 'tasks']);
      return [t('own'), t('badge'), otherT('own'), t('common:badge'), t('badge', { ns: 'common' })];
    }
  `,
    dictionaries,
  );
  assert.equal(report.status, 1);
  assert.deepEqual(
    report.findings
      .filter((finding) => finding.kind === "missing-en")
      .map((finding) => finding.value),
    ["tasks:badge", "common:own"],
  );
  const fallback = inspect(
    `
    function C() {
      const { t } = useTranslation(['tasks', 'common'], { nsMode: 'fallback' });
      return [t('own'), t('badge')];
    }
  `,
    dictionaries,
  );
  assert.equal(fallback.status, 0);
});

test("checks every finite typed dynamic key and retains arbitrary strings as unresolved", () => {
  const source = `function C(status: 'ready' | 'failed', flag: boolean, raw: string) {
    const { t } = useTranslation('tasks');
    const key = \`state.\${status}\` as const;
    return [t(key), t(flag ? 'common:save' : 'common:cancel'), t(raw)];
  }`;
  const en = {
    tasks: { state: { ready: "Ready", failed: "Failed" } },
    common: { save: "Save", cancel: "Cancel" },
  };
  const valid = inspect(source, en);
  assert.equal(valid.counts.dynamic, 3);
  assert.equal(valid.counts.resolvedDynamic, 2);
  assert.equal(valid.summary["dynamic-key"], 1);
  assert.equal(valid.summary["missing-en"], undefined);
  delete en.tasks.state.failed;
  const missing = inspect(source, en);
  assert(
    missing.findings.some(
      (f) => f.kind === "missing-en" && f.value === "tasks:state.failed",
    ),
  );
  assert.equal(missing.summary["dynamic-key"], 1);
});

test("permits incomplete optional locales while checking their JSON syntax and duplicate keys", () => {
  const en = { common: { saved: "Saved" } };
  const inspectFrench = (common) =>
    inspect("", en, undefined, [], {}, { fr: { common } });
  assert.equal(inspectFrench({}).status, 0);
  const duplicate = inspectFrench(
    '{"saved":"Enregistré","saved":"Sauvegardé"}',
  );
  assert.equal(duplicate.summary.duplicate, 1);
  assert.equal(
    duplicate.findings[0].file,
    "packages/web-core/src/i18n/locales/fr/common.json",
  );
  assert.equal(inspectFrench("{").summary["invalid-json"], 1);
  assert.equal(inspectFrench("[]").summary["invalid-json"], 1);
});

test("unused inventory distinguishes namespace static references from dynamic prefixes and keeps plural/schema references", () => {
  const report = inspect(
    `
    i18n.t('tasks:active.heading');
    i18n.t('tasks:items', {count: 2});
    i18n.t(\`tasks:states.\${state}\`);
  `,
    {
      tasks: {
        active: { heading: "Heading", obsolete: "Old" },
        items_one: "One",
        items_other: "Many",
        states: { ready: "Ready" },
        schemaTitle: "Model",
      },
    },
    undefined,
    [],
    {
      codex: {
        properties: { model: { "x-lvk-i18n-title": "tasks:schemaTitle" } },
      },
    },
    {},
    "scripts/check-unused-i18n-keys.mjs",
  );
  assert.equal(report.status, 1);
  assert.deepEqual(report.unusedByNs, { tasks: ["active.obsolete"] });
});

test("rejects Japanese-only stale keys while preserving locale plural differences", () => {
  const report = inspect(
    "",
    { common: { items_one: "One item", items_other: "Many items" } },
    {
      common: { items: "項目", stale: "古い項目" },
      removed: { old: "旧項目" },
    },
  );
  assert.equal(report.status, 1);
  assert.deepEqual(
    report.findings
      .filter((f) => f.kind === "extra-ja")
      .map((f) => `${f.file}:${f.value}`),
    ["common:stale", "removed:old"],
  );
  assert.equal(report.summary["missing-ja"], undefined);
});

test("leaves identical translations and indirect UI strings outside the hardcode gate", () => {
  const report = inspect(
    'const text = "Delete all workspaces"; function C() { return <button title="Help">{text}</button>; }',
    { common: { save: "Save" } },
  );
  assert.equal(report.status, 0);
});
