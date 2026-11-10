import assert from 'node:assert/strict';
import { test } from 'node:test';
import { readFileSync } from 'node:fs';
import { transformSync } from 'esbuild';
import { createRequire } from 'node:module';
const require = createRequire(
  `${process.cwd()}/packages/web-core/package.json`
);
const { createEditor, $getRoot, $createTextNode } = require('lexical');
const { HeadingNode, QuoteNode } = require('@lexical/rich-text');
const { ListNode, ListItemNode } = require('@lexical/list');
const { CodeNode, CodeHighlightNode } = require('@lexical/code');
const { LinkNode } = require('@lexical/link');
const {
  TRANSFORMERS,
  $convertFromMarkdownString,
  $convertToMarkdownString,
} = require('@lexical/markdown');
const compiled = transformSync(
  readFileSync('packages/ui/src/lib/markdownQuote.ts', 'utf8'),
  { loader: 'ts', format: 'cjs' }
).code;
const adapter = { exports: {} };
new Function('require', 'module', 'exports', compiled)(
  require,
  adapter,
  adapter.exports
);
const transformers = [adapter.exports.MARKDOWN_QUOTE, ...TRANSFORMERS];
for (const marker of [
  'DECISION',
  'NOTE',
  'TIP',
  'IMPORTANT',
  'WARNING',
  'CAUTION',
]) {
  test(`${marker} survives import, user edit, export and reimport`, () => {
    const editor = createEditor({
      nodes: [
        HeadingNode,
        QuoteNode,
        ListNode,
        ListItemNode,
        CodeNode,
        CodeHighlightNode,
        LinkNode,
      ],
      onError: (e) => {
        throw e;
      },
    });
    const source = `> [!${marker}]\n> 日本語の判断事項\n> ソフト改行\n> HardOne  \n> HardTwo\n>\n> 複数段落の補足\n\n> **人間の意図**\n> 通常引用を保つ`;
    let output;
    editor.update(
      () => {
        $convertFromMarkdownString(source, transformers);
        $getRoot().getFirstChild().append($createTextNode(' 編集後'));
        output = $convertToMarkdownString(transformers);
        assert.ok(output.includes(`> [!${marker}]`), output);
        assert.ok(output.includes('**人間の意図**'), output);
        assert.ok(output.includes('> HardOne  \n> HardTwo'), output);
        assert.ok(!output.includes('> >'), output);
        assert.ok(output.includes('> \n> 複数段落の補足'), output);
        $convertFromMarkdownString(output, transformers);
        assert.equal($convertToMarkdownString(transformers), output);
      },
      { discrete: true }
    );
  });
}
