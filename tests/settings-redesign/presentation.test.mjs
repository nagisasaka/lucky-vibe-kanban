import { createRequire } from 'node:module';
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import assert from 'node:assert/strict';
import test from 'node:test';
const require = createRequire(
  new URL('../../packages/local-web/package.json', import.meta.url)
);
const ts = require('typescript');
const baseline = '43774ca91388b9b4b0f2b78f23e3099f1eea6825';
const root = 'packages/web-core/src/shared/dialogs/settings/';
// Ignore presentation attributes and JSX formatting, retaining text, attributes,
// control flow and handlers. This redesign must not remove settings or semantics.
function semantics(source) {
  const file = ts.createSourceFile(
    'settings.tsx',
    source,
    ts.ScriptTarget.Latest,
    true,
    ts.ScriptKind.TSX
  );
  function visit(node) {
    if (ts.isParenthesizedExpression(node)) return visit(node.expression);
    if (ts.isJsxAttribute(node) && node.name.getText(file) === 'className')
      return null;
    if (ts.isJsxText(node)) {
      const text = node.text.replace(/\s+/g, ' ').trim();
      return text ? ['text', text] : null;
    }
    const children = [];
    ts.forEachChild(node, (child) => {
      const value = visit(child);
      if (value) children.push(value);
    });
    return [
      node.kind,
      ts.isStringLiteral(node) || ts.isNumericLiteral(node)
        ? node.text
        : ts.isIdentifier(node)
          ? node.text
          : '',
      children,
    ];
  }
  return visit(file);
}
for (const path of [
  'SettingsDialog.tsx',
  'settings/SettingsSection.tsx',
  'settings/SettingsComponents.tsx',
]) {
  test(`${path}: all content, controls, guards and handlers survive presentation changes`, () => {
    const file = root + path;
    const before = execFileSync('git', ['show', `${baseline}:${file}`], {
      encoding: 'utf8',
    });
    assert.deepEqual(semantics(readFileSync(file, 'utf8')), semantics(before));
  });
}
test('all section implementations and registry retain their baseline content', () => {
  const files = execFileSync(
    'git',
    ['ls-tree', '-r', '--name-only', baseline, root + 'settings'],
    { encoding: 'utf8' }
  )
    .trim()
    .split('\n')
    .filter(
      (file) =>
        /\.tsx?$/.test(file) &&
        !file.endsWith('/SettingsComponents.tsx') &&
        !file.endsWith('/SettingsSection.tsx')
    );
  for (const file of files)
    assert.equal(
      readFileSync(file, 'utf8'),
      execFileSync('git', ['show', `${baseline}:${file}`], {
        encoding: 'utf8',
      }),
      file
    );
});

test('login redesign preserves translated copy, authentication handlers and security attributes', () => {
  const path='packages/local-web/src/app/auth/ServerAccessGate.tsx';
  function contract(source) {
    const file=ts.createSourceFile('access.tsx',source,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
    const calls=[],attributes=[];
    const printer=ts.createPrinter({removeComments:true});
    const print=node=>printer.printNode(ts.EmitHint.Unspecified,node,file).replace(/\s+/g,' ');
    function visit(node) {
      if(ts.isCallExpression(node) && ts.isIdentifier(node.expression) && ['t','accessRequest','loadAccessStatus','navigateAfterSuccess','rememberPushReturn','takePushReturn'].includes(node.expression.text)) calls.push(print(node));
      if(ts.isJsxAttribute(node) && /^(on[A-Z]|disabled|required|type$|autoComplete$|href$|readOnly$|value$|checked$)/.test(node.name.getText(file))) attributes.push(print(node));
      ts.forEachChild(node,visit);
    }
    visit(file);return {calls,attributes};
  }
  assert.deepEqual(contract(readFileSync(path,'utf8')),contract(execFileSync('git',['show',`${baseline}:${path}`],{encoding:'utf8'})));
});
