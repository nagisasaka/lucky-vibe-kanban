# Project transfer browser fixture

Run from the repository root with installed workspace dependencies:

```sh
node tests/project-transfer/browser.mjs
```

Set `CHROMIUM_PATH` when Chromium is installed elsewhere. The default is the
isolated CI headless browser at
`/opt/lvk-browsers/chromium-1217/chrome-linux64/chrome`.

The fixture bundles the real transfer panel, transfer API client and imported
Issue editor. Playwright intercepts every request with synthetic HTTP responses;
it does not start an LVK server, read a database or use a shared browser. The
uploaded bytes are synthetic, so SQLite validation and actual database
transactions are covered separately by backend tests.

Checks include explicit history opt-in, preview/import byte and digest binding,
required key/repository mapping, replacement file/stale preview handling,
persisted attachment omission reports, inert archived conversation/Manifest
viewing, remapped structured Issue links, literal imported Issue viewing/editing,
and light/dark layouts at mobile/desktop widths. Screenshots and a small request
summary are written to the ignored task output `.build/project-transfer-browser`.
The headless browser is closed in `finally`.
