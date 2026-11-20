import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

// Reads the shipped theme tokens and checks WCAG 2.x contrast for the main
// text/surface combinations of every colour theme.
const css = readFileSync(
  new URL('../../app/styles/new/index.css', import.meta.url),
  'utf8'
);
const diffCss = readFileSync(
  new URL('../../../../ui/src/styles/diff-style-overrides.css', import.meta.url),
  'utf8'
);

type Tokens = Record<string, string>;

function block(source: string, selector: string): Tokens {
  const start = source.indexOf(`${selector} {`);
  if (start < 0) throw new Error(`missing block ${selector}`);
  const body = source.slice(
    source.indexOf('{', start) + 1,
    source.indexOf('}', start)
  );
  const tokens: Tokens = {};
  for (const match of body.matchAll(/(--[\w-]+):\s*([^;]+);/g)) {
    tokens[match[1]] = match[2].trim();
  }
  return tokens;
}

const light = {
  ...block(diffCss, '.light'),
  ...block(css, ':root,\n  .light'),
};
const dark = { ...block(diffCss, '.dark'), ...block(css, '  .dark') };
const THEMES: Record<string, Tokens> = {
  'light/standard': light,
  'light/paper': {
    ...light,
    ...block(css, ".light[data-color-theme='paper']"),
  },
  'light/linen': {
    ...light,
    ...block(css, ".light[data-color-theme='linen']"),
  },
  'dark/standard': dark,
  'dark/navy': { ...dark, ...block(css, ".dark[data-color-theme='navy']") },
  'dark/indigo': { ...dark, ...block(css, ".dark[data-color-theme='indigo']") },
};

type Rgb = [number, number, number];

function rgb(tokens: Tokens, name: string): Rgb {
  const value = tokens[name];
  const match = value?.match(/^([\d.]+) ([\d.]+)% ([\d.]+)%$/);
  if (!match) throw new Error(`${name} is not an HSL token: ${value}`);
  const [h, s, l] = [
    Number(match[1]),
    Number(match[2]) / 100,
    Number(match[3]) / 100,
  ];
  const a = s * Math.min(l, 1 - l);
  const f = (n: number) => {
    const k = (n + h / 30) % 12;
    return l - a * Math.max(-1, Math.min(k - 3, 9 - k, 1));
  };
  return [f(0), f(8), f(4)];
}

function luminance([r, g, b]: Rgb) {
  const channel = (c: number) =>
    c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);
}

function contrast(a: Rgb, b: Rgb) {
  const [high, low] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (high + 0.05) / (low + 0.05);
}

function over(top: Rgb, alpha: number, base: Rgb): Rgb {
  return top.map((c, i) => c * alpha + base[i] * (1 - alpha)) as Rgb;
}

const SURFACES = [
  '--_bg-primary-default',
  '--_bg-secondary-default',
  '--_bg-panel-default',
];
const TEXT = 4.5;
const NON_TEXT = 3;

describe.each(Object.entries(THEMES))('%s contrast', (_name, tokens) => {
  const c = (fg: string, bg: string) =>
    contrast(rgb(tokens, fg), rgb(tokens, bg));

  it('keeps text readable on every surface', () => {
    for (const surface of SURFACES) {
      for (const text of [
        '--text-high',
        '--text-normal',
        '--text-low',
        '--brand-text',
      ]) {
        expect(
          c(text, surface),
          `${text} on ${surface}`
        ).toBeGreaterThanOrEqual(TEXT);
      }
    }
  });

  it('keeps status colours readable on surfaces', () => {
    for (const surface of ['--_bg-primary-default', '--_bg-panel-default']) {
      for (const status of [
        '--error',
        '--_success',
        '--_warning',
        '--_info',
        '--merged',
      ]) {
        expect(
          c(status, surface),
          `${status} on ${surface}`
        ).toBeGreaterThanOrEqual(TEXT);
      }
    }
  });

  it('uses readable text on filled buttons', () => {
    const pairs: [string, string][] = [
      ['--text-on-brand', '--brand'],
      ['--text-on-brand', '--brand-hover'],
      ['--text-on-brand-secondary', '--brand-secondary'],
      ['--text-on-brand-secondary', '--brand-secondary-hover'],
      ['--text-on-error', '--error'],
      ['--_destructive-foreground', '--_destructive'],
      ['--text-on-success', '--_success'],
    ];
    for (const [fg, bg] of pairs) {
      expect(c(fg, bg), `${fg} on ${bg}`).toBeGreaterThanOrEqual(TEXT);
    }
  });

  it('keeps shared orange fills and visible focus and selections', () => {
    expect(tokens['--brand']).toBe('25 82% 54%');
    expect(tokens['--brand-hover']).toBe('25 75% 62%');
    for (const surface of [
      '--_bg-primary-default',
      '--_bg-secondary-default',
    ]) {
      expect(
        c('--brand-text', surface),
        `focus ring on ${surface}`
      ).toBeGreaterThanOrEqual(NON_TEXT);
    }
    const selection = over(
      rgb(tokens, '--brand'),
      0.2,
      rgb(tokens, '--_bg-primary-default')
    );
    expect(
      contrast(rgb(tokens, '--text-low'), selection)
    ).toBeGreaterThanOrEqual(TEXT);
    expect(
      contrast(rgb(tokens, '--text-high'), selection)
    ).toBeGreaterThanOrEqual(TEXT);
  });

  it('keeps diff and console text readable', () => {
    expect(c('--text-normal', '--line-added')).toBeGreaterThanOrEqual(TEXT);
    expect(c('--text-normal', '--line-removed')).toBeGreaterThanOrEqual(TEXT);
    expect(
      c('--_console-foreground', '--_console-background')
    ).toBeGreaterThanOrEqual(TEXT);
  });
});
