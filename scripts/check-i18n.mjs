#!/usr/bin/env node
// Inspect all owned frontend packages. Inventory mode retains unresolved findings;
// strict mode is intended for CI once the release-quality migration is complete.
import fs from "node:fs";
import path from "node:path";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
const rootIndex = process.argv.indexOf("--root");
const root =
  rootIndex < 0
    ? fileURLToPath(new URL("../", import.meta.url))
    : path.resolve(process.argv[rootIndex + 1]);
const require = createRequire(
  new URL("../packages/web-core/package.json", import.meta.url),
);
const ts = require("typescript");
const findings = [];
const counts = {
  files: 0,
  calls: 0,
  dynamic: 0,
  resolvedDynamic: 0,
  defaultValues: 0,
};
const add = (kind, file, line, value) =>
  findings.push({ kind, file, line, value });
const flatten = (value, prefix = "", out = {}) => {
  for (const [key, item] of Object.entries(value)) {
    const name = prefix ? `${prefix}.${key}` : key;
    if (item && typeof item === "object") flatten(item, name, out);
    else out[name] = item;
  }
  return out;
};
const locales = {};
const localeDirectory = path.join(root, "packages/web-core/src/i18n/locales");
// Optional translations may be incomplete; every shipped JSON must stay valid.
for (const language of fs
  .readdirSync(localeDirectory, { withFileTypes: true })
  .filter((entry) => entry.isDirectory())
  .map((entry) => entry.name)) {
  locales[language] = {};
  const dir = path.join(root, "packages/web-core/src/i18n/locales", language);
  for (const filename of fs
    .readdirSync(dir)
    .filter((f) => f.endsWith(".json"))) {
    const file = path.join(dir, filename);
    const content = fs.readFileSync(file, "utf8");
    const source = ts.parseJsonText(file, content);
    function duplicates(node) {
      if (ts.isObjectLiteralExpression(node)) {
        const seen = new Set();
        for (const prop of node.properties) {
          const name = prop.name?.text;
          if (seen.has(name))
            add(
              "duplicate",
              path.relative(root, file),
              source.getLineAndCharacterOfPosition(prop.getStart(source)).line +
                1,
              name,
            );
          seen.add(name);
        }
      }
      ts.forEachChild(node, duplicates);
    }
    duplicates(source);
    let parsed;
    try {
      parsed = JSON.parse(content);
    } catch {
      add(
        "invalid-json",
        path.relative(root, file),
        0,
        "Locale file must contain valid JSON",
      );
      continue;
    }
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
      add(
        "invalid-json",
        path.relative(root, file),
        0,
        "Locale namespace must be a JSON object",
      );
      continue;
    }
    locales[language][filename.slice(0, -5)] = flatten(parsed);
  }
}
const placeholders = (s) =>
  [...s.matchAll(/{{\s*-?\s*([^},\s]+)(?:[^}]*)}}/g)]
    .map((m) => m[1])
    .sort()
    .join(",");
for (const [ns, english] of Object.entries(locales.en)) {
  const japanese = locales.ja[ns] || {};
  for (const [key, value] of Object.entries(english)) {
    const jaKey = key in japanese ? key : key.replace(/_(one|other)$/, "");
    if (!(jaKey in japanese)) add("missing-ja", ns, 0, key);
    else if (
      typeof value === "string" &&
      value === japanese[jaKey] &&
      /[A-Za-z]/.test(value.replace(/{{[^}]+}}/g, ""))
    ) {
      add(
        "same-en-ja",
        `packages/web-core/src/i18n/locales/ja/${ns}.json`,
        0,
        `${key}: ${value}`,
      );
    }
    if (
      typeof value === "string" &&
      typeof japanese[jaKey] === "string" &&
      placeholders(value) !== placeholders(japanese[jaKey])
    )
      add("interpolation", ns, 0, key);
  }
  for (const language of ["en", "ja"])
    for (const [key, value] of Object.entries(locales[language][ns] || {})) {
      if (typeof value !== "string" || !value.trim())
        add("empty-or-invalid", `${language}/${ns}`, 0, key);
    }
}
function walkFiles(dir) {
  return fs.readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const file = path.join(dir, entry.name);
    return entry.isDirectory()
      ? walkFiles(file)
      : /\.(ts|tsx)$/.test(file) && !/\.(test|stories|d)\.tsx?$/.test(file)
        ? [file]
        : [];
  });
}
const packageFiles = new Map(
  ["local-web", "web-core", "ui"].map((pkg) => [
    pkg,
    walkFiles(path.join(root, "packages", pkg, "src")),
  ]),
);
const typePrograms = new Map();
function finiteStringValues(type) {
  if (type.flags & ts.TypeFlags.StringLiteral) return [type.value];
  if (type.isUnion() && type.types.length <= 256) {
    const values = type.types.map(finiteStringValues);
    if (values.every(Boolean) && values.flat().length <= 256)
      return [...new Set(values.flat())];
  }
  return null;
}
function finiteKeys(pkg, file, node, source) {
  // Use each owner's actual config. A broad string/any/template pattern stays unresolved.
  if (!typePrograms.has(pkg)) {
    const directory = path.join(root, "packages", pkg);
    const configPath = path.join(directory, "tsconfig.json");
    let options = {
      target: ts.ScriptTarget.ESNext,
      jsx: ts.JsxEmit.Preserve,
      strict: true,
      noEmit: true,
    };
    let rootNames = packageFiles.get(pkg);
    if (fs.existsSync(configPath)) {
      const config = ts.readConfigFile(configPath, ts.sys.readFile);
      if (config.error)
        throw new Error(
          ts.flattenDiagnosticMessageText(config.error.messageText, "\n"),
        );
      const parsed = ts.parseJsonConfigFileContent(
        config.config,
        ts.sys,
        directory,
      );
      if (parsed.errors.length)
        throw new Error(
          parsed.errors
            .map((e) => ts.flattenDiagnosticMessageText(e.messageText, "\n"))
            .join("\n"),
        );
      options = parsed.options;
      // Ambient declarations and configured roots affect the types of scanned expressions.
      rootNames = [...new Set([...parsed.fileNames, ...rootNames])];
    }
    const program = ts.createProgram(rootNames, {
      ...options,
      noEmit: true,
    });
    typePrograms.set(pkg, {
      program,
      checker: program.getTypeChecker(),
      nodes: new Map(),
    });
  }
  const context = typePrograms.get(pkg);
  const typedSource = context.program.getSourceFile(file);
  if (!typedSource) return null;
  if (!context.nodes.has(file)) {
    const nodes = new Map();
    function collect(n) {
      if (ts.isCallExpression(n) && n.arguments[0])
        nodes.set(n.arguments[0].getStart(typedSource), n.arguments[0]);
      ts.forEachChild(n, collect);
    }
    collect(typedSource);
    context.nodes.set(file, nodes);
  }
  const typedNode = context.nodes.get(file).get(node.getStart(source));
  if (!typedNode || typedNode.getText(typedSource) !== node.getText(source))
    return null;
  return finiteStringValues(context.checker.getTypeAtLocation(typedNode));
}
for (const [pkg, files] of packageFiles)
  for (const file of files) {
    const rel = path.relative(root, file);
    const source = ts.createSourceFile(
      file,
      fs.readFileSync(file, "utf8"),
      ts.ScriptTarget.Latest,
      true,
    );
    counts.files++;
    const literal = (n) =>
      n && (ts.isStringLiteral(n) || ts.isNoSubstitutionTemplateLiteral(n))
        ? n.text
        : undefined;
    const location = (n) =>
      source.getLineAndCharacterOfPosition(n.getStart(source)).line + 1;
    // Resolve the hook in the enclosing function, never a different component's namespace.
    function namespaces(node, callee, depth = 0) {
      if (depth > 4) return [];
      for (let scope = node.parent; scope; scope = scope.parent) {
        if (!ts.isFunctionLike(scope) && !ts.isSourceFile(scope)) continue;
        let result;
        function visit(n) {
          if (n !== scope && ts.isFunctionLike(n)) return;
          if (
            ts.isCallExpression(n) &&
            n.expression.getText(source) === "useTranslation"
          ) {
            const declaration = n.parent;
            if (
              !ts.isVariableDeclaration(declaration) ||
              !ts.isObjectBindingPattern(declaration.name)
            )
              return;
            const binding = declaration.name.elements.find(
              (element) =>
                (element.propertyName?.getText(source) ||
                  element.name.getText(source)) === "t" &&
                element.name.getText(source) === callee,
            );
            if (!binding) return;
            const arg = n.arguments[0];
            if (ts.isArrayLiteralExpression(arg ?? {})) {
              const declared = arg.elements.map(literal).filter(Boolean);
              const options = n.arguments[1];
              const mode =
                options && ts.isObjectLiteralExpression(options)
                  ? options.properties.find(
                      (property) =>
                        ts.isPropertyAssignment(property) &&
                        property.name.getText(source) === "nsMode",
                    )
                  : undefined;
              // react-i18next binds the first namespace by default. Only its
              // explicit fallback mode binds the full namespace list.
              result =
                literal(mode?.initializer) === "fallback"
                  ? declared
                  : [declared[0] || "common"];
            } else result = [literal(arg) || "common"];
          }
          ts.forEachChild(n, visit);
        }
        visit(scope);
        if (result) return result;
        if (ts.isFunctionDeclaration(scope) && scope.name) {
          const index = scope.parameters.findIndex(
            (p) => p.name.getText(source) === callee,
          );
          if (index >= 0) {
            const resolved = new Set();
            function callers(n) {
              if (
                ts.isCallExpression(n) &&
                n.expression.getText(source) === scope.name.text &&
                n.arguments[index]
              ) {
                for (const ns of namespaces(
                  n,
                  n.arguments[index].getText(source),
                  depth + 1,
                ))
                  resolved.add(ns);
              }
              ts.forEachChild(n, callers);
            }
            callers(source);
            if (resolved.size) return [...resolved];
          }
        }
      }
      return ["common"];
    }
    function visit(node) {
      if (
        ts.isCallExpression(node) &&
        /^(?:t|\w+T|i18n\.t)$/.test(node.expression.getText(source))
      ) {
        counts.calls++;
        const key = literal(node.arguments[0]);
        const options = node.arguments.find(ts.isObjectLiteralExpression);
        if (
          node.arguments.length > 1 &&
          (literal(node.arguments[1]) !== undefined ||
            options?.properties.some(
              (p) => p.name?.getText(source) === "defaultValue",
            ))
        )
          counts.defaultValues++;
        const keys =
          key === undefined && node.arguments[0]
            ? finiteKeys(pkg, file, node.arguments[0], source)
            : key === undefined
              ? null
              : [key];
        if (key === undefined) counts.dynamic++;
        if (!keys) {
          add(
            "dynamic-key",
            rel,
            location(node),
            node.arguments[0]?.getText(source),
          );
        } else {
          if (key === undefined) counts.resolvedDynamic++;
          for (const key of keys) {
            const explicitNs = options?.properties.find(
              (p) => p.name?.getText(source) === "ns",
            );
            const ns = key.includes(":")
              ? [key.split(":")[0]]
              : explicitNs
                ? [literal(explicitNs.initializer)]
                : namespaces(node, node.expression.getText(source));
            const name = key.includes(":")
              ? key.slice(key.indexOf(":") + 1)
              : key;
            if (
              !ns.some(
                (n) =>
                  locales.en[n]?.[name] !== undefined ||
                  locales.en[n]?.[`${name}_other`] !== undefined,
              )
            )
              add("missing-en", rel, location(node), `${ns.join("|")}:${name}`);
          }
        }
      }
      if (ts.isJsxText(node) && /[A-Za-z\u3040-\u9fff]/.test(node.text.trim()))
        add(
          "hardcode-markup",
          rel,
          location(node),
          node.text.trim().replace(/\s+/g, " "),
        );
      if (
        ts.isStringLiteral(node) ||
        ts.isNoSubstitutionTemplateLiteral(node) ||
        ts.isTemplateExpression(node)
      ) {
        const text = ts.isTemplateExpression(node)
          ? node.head.text +
            node.templateSpans.map((span) => span.literal.text).join("")
          : node.text;
        const value = ts.isTemplateExpression(node)
          ? node.getText(source)
          : node.text;
        let context = node.parent;
        while (
          ts.isConditionalExpression(context) ||
          ts.isParenthesizedExpression(context) ||
          ts.isJsxExpression(context) ||
          ts.isArrayLiteralExpression(context) ||
          (ts.isPropertyAccessExpression(context) &&
            ["filter", "join"].includes(context.name.text)) ||
          (ts.isCallExpression(context) &&
            ts.isPropertyAccessExpression(context.expression) &&
            ["filter", "join"].includes(context.expression.name.text)) ||
          (ts.isBinaryExpression(context) &&
            [
              ts.SyntaxKind.PlusToken,
              ts.SyntaxKind.BarBarToken,
              ts.SyntaxKind.QuestionQuestionToken,
            ].includes(context.operatorToken.kind)) ||
          ts.isArrowFunction(context) ||
          ts.isFunctionExpression(context) ||
          (ts.isReturnStatement(context) &&
            ts.isBlock(context.parent) &&
            ts.isFunctionLike(context.parent.parent))
        ) {
          context = ts.isReturnStatement(context)
            ? context.parent.parent
            : context.parent;
        }
        const attr = ts.isJsxAttribute(context)
          ? context.name.getText(source)
          : undefined;
        const declaration =
          ts.isBindingElement(context) ||
          ts.isParameter(context) ||
          ts.isVariableDeclaration(context) ||
          ts.isFunctionDeclaration(context) ||
          ts.isMethodDeclaration(context)
            ? context
            : undefined;
        const declarationName =
          declaration &&
          (ts.isBindingElement(declaration) && declaration.propertyName
            ? declaration.propertyName
            : declaration.name);
        // A destructured property alias still belongs to the original UI slot.
        // IDs, paths and other data defaults are deliberately not label slots.
        const slot =
          declarationName &&
          (ts.isIdentifier(declarationName) ||
            ts.isStringLiteral(declarationName))
            ? declarationName.text.replace(/^get([A-Z])/, (_, c) =>
                c.toLowerCase(),
              )
            : undefined;
        const property = ts.isPropertyAssignment(context)
          ? context.name.getText(source)
          : slot;
        if (
          /[A-Za-z\u3040-\u9fff]/.test(text) &&
          ([
            "title",
            "label",
            "description",
            "message",
            "placeholder",
            "aria-label",
            "alt",
          ].includes(attr) ||
            [
              "title",
              "label",
              "description",
              "message",
              "placeholder",
              "tooltip",
              "emptyText",
              "emptyMessage",
              "errorMessage",
              "metadataLine",
              "confirmText",
              "cancelText",
            ].includes(property))
        )
          add("hardcode-label", rel, location(node), value);
        if (
          /[A-Za-z\u3040-\u9fff]/.test(text) &&
          (ts.isJsxElement(context) || ts.isJsxFragment(context))
        )
          add("hardcode-markup", rel, location(node), value);
        if (
          (ts.isCallExpression(context) || ts.isNewExpression(context)) &&
          context.arguments?.[0] &&
          context.arguments[0].pos <= node.pos &&
          context.arguments[0].end >= node.end
        ) {
          const callee = context.expression.getText(source);
          const messageCall =
            /(?:^|\.)(?:alert|confirm|prompt|set(?:[A-Z][A-Za-z]*)?(?:Error|Notice|Message)|toast|success|error|info|warning)$/.test(
              callee,
            ) && !callee.startsWith("console.");
          const errorConstructor =
            ts.isNewExpression(context) && /^(?:Error|ApiError)$/.test(callee);
          const prose =
            /[\s.!?。…ぁ-んァ-ヶ一-龯]/.test(text) ||
            /^[A-Z][A-Za-z]+$/.test(text);
          if (
            (messageCall && prose && /[A-Za-z\u3040-\u9fff]/.test(text)) ||
            (errorConstructor && /[A-Za-z\u3040-\u9fff]/.test(text))
          )
            add("hardcode-message", rel, location(node), value);
        }
      }
      ts.forEachChild(node, visit);
    }
    visit(source);
  }
// Generated executor schema annotations are dynamic translation callsites.
const schemaDirectory = path.join(root, "shared/schemas");
if (fs.existsSync(schemaDirectory)) {
  for (const filename of fs
    .readdirSync(schemaDirectory)
    .filter((name) => name.endsWith(".json"))) {
    const file = path.join(schemaDirectory, filename);
    const schema = JSON.parse(fs.readFileSync(file, "utf8"));
    for (const [property, field] of Object.entries(schema.properties ?? {})) {
      if (Array.isArray(field.enum)) {
        const labels = field["x-lvk-i18n-enum"];
        if (!Array.isArray(labels) || labels.length !== field.enum.length) {
          add("schema-enum-labels", path.relative(root, file), 0, property);
        } else
          labels.forEach((label, index) => {
            const canonical =
              field.enum[index] === null ? "null" : field.enum[index];
            const expected = `settings:executorSchemas.${filename.slice(0, -5)}.${property}.options.${canonical}`;
            if (label.key !== expected)
              add("schema-i18n-key", path.relative(root, file), 0, expected);
            const [namespace, name] = expected.split(":");
            if (locales.en[namespace]?.[name] !== label.source)
              add(
                "schema-source-drift",
                path.relative(root, file),
                0,
                expected,
              );
            if (!locales.ja[namespace]?.[name])
              add("schema-missing-ja", path.relative(root, file), 0, expected);
          });
      }
      for (const annotation of ["title", "description"]) {
        if (
          annotation === "description" &&
          typeof field.description !== "string"
        )
          continue;
        const key = field[`x-lvk-i18n-${annotation}`];
        const expected = `settings:executorSchemas.${filename.slice(0, -5)}.${property}.${annotation}`;
        if (key !== expected) {
          add("schema-i18n-key", path.relative(root, file), 0, expected);
          continue;
        }
        const [namespace, name] = key.split(":");
        if (locales.en[namespace]?.[name] !== field[annotation])
          add("schema-source-drift", path.relative(root, file), 0, key);
        if (!locales.ja[namespace]?.[name])
          add("schema-missing-ja", path.relative(root, file), 0, key);
      }
    }
  }
}
// Exceptions name exact tokens or generated-schema call expressions with a
// separately checked annotation contract, never directories or whole files.
const allowlistPath = path.join(root, "scripts/i18n-allowlist.json");
const allowlist = fs.existsSync(allowlistPath)
  ? JSON.parse(fs.readFileSync(allowlistPath, "utf8"))
  : [];
const allowlisted = [];
for (const entry of allowlist) {
  if (
    ![
      "hardcode-label",
      "hardcode-markup",
      "hardcode-message",
      "same-en-ja",
      "dynamic-key",
    ].includes(entry.kind) ||
    typeof entry.file !== "string" ||
    entry.file.includes("*") ||
    typeof entry.value !== "string" ||
    !entry.value ||
    typeof entry.reason !== "string" ||
    !entry.reason.trim()
  ) {
    add(
      "invalid-allowlist",
      "scripts/i18n-allowlist.json",
      0,
      JSON.stringify(entry),
    );
    continue;
  }
  let matched = false;
  for (let index = findings.length - 1; index >= 0; index--) {
    const finding = findings[index];
    if (
      finding.kind === entry.kind &&
      finding.file === entry.file &&
      finding.value === entry.value
    ) {
      matched = true;
      allowlisted.push({ ...finding, reason: entry.reason });
      findings.splice(index, 1);
    }
  }
  if (!matched) add("stale-allowlist", entry.file, 0, entry.value);
}
const report = {
  counts,
  allowlisted,
  summary: Object.fromEntries(
    [...new Set(findings.map((f) => f.kind))].map((k) => [
      k,
      findings.filter((f) => f.kind === k).length,
    ]),
  ),
  findings,
};
if (process.argv.includes("--json"))
  console.log(JSON.stringify(report, null, 2));
else {
  console.log(JSON.stringify({ counts, summary: report.summary }, null, 2));
  for (const f of findings)
    console.log(`${f.kind} ${f.file}:${f.line} ${f.value}`);
}
if (!process.argv.includes("--inventory") && findings.length)
  process.exitCode = 1;
