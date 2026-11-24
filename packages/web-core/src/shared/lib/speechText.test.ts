import { describe, expect, it } from 'vitest';
import { speechText } from './speechText';

describe('speechText', () => {
  it.each([
    ['<div>本文<br>次の行 &amp; 結論</div>', '本文 次の行 & 結論'],
    ['<script>hidden()</script>\n\n本文', '本文'],
    [
      '**確認中**です。`file.ts`と[仕様](https://example.com)を確認。',
      '確認中 です。 file.ts と 仕様 を確認。',
    ],
    [
      '# 見出し\n\n> 引用\n\n- **項目**\n- [x] 完了\n\n~~削除~~',
      '見出し 引用 項目 完了 削除',
    ],
    [
      'C++ と `a * b`、`x ** 2`、`foo_bar`',
      'C++ と a * b 、 x ** 2 、 foo_bar',
    ],
    [
      '\\*文字どおり\\* と a * b と 2 + 3 = 5',
      '*文字どおり* と a * b と 2 + 3 = 5',
    ],
    ['```cpp\nC++ a * b\n```', 'C++ a * b'],
    [
      'one**two**three`four`five[six](https://example.com)seven',
      'one two three four five six seven',
    ],
    ['[仕様][ref]\n\n[ref]: https://example.com "タイトル"', '仕様'],
    ['| 名前 | 値 |\n| --- | --- |\n| a | `b * c` |', '名前 値 a b * c'],
    ['&amp; <kbd>Enter</kbd> ![説明](image.png)\n\n---', '& Enter 説明'],
  ])('converts %s without removing literal symbols', (input, expected) => {
    expect(speechText(input)).toBe(expected);
  });
});
