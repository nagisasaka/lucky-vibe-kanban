#!/usr/bin/env node
// LVK contributor entry point. Never export compiler settings into the server.
import { existsSync, realpathSync, mkdirSync } from 'node:fs';
import { dirname, join, resolve, delimiter } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync, spawn } from 'node:child_process';

const root = realpathSync(join(dirname(fileURLToPath(import.meta.url)), '..'));
const wrapper = join(root, 'scripts/dev-rustc-cache.mjs');
const args = process.argv.slice(2);
const fail = (message) => { console.error(`dev-cargo: ${message}`); process.exit(2); };
const warn = (message) => console.error(`dev-cargo: ${message}`);
const quote = (value) => `'${value.replaceAll("'", "'\\''")}'`;
const executable = (name) => (process.env.PATH || '').split(delimiter)
  .map((dir) => join(dir, name)).find((path) => existsSync(path));

if (args[0] === 'watch') {
  if (args.length !== 1) fail('watch accepts no arguments; use backend:dev:watch.');
  const entry = `${quote(process.execPath)} ${quote(fileURLToPath(import.meta.url))}`;
  const child = spawn('cargo', ['watch', '-w', 'crates', '-s',
    `${entry} build --bin server --bin agent-process-host && ${entry} run --bin server`],
  { cwd: root, stdio: 'inherit' });
  for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, () => child.kill(signal));
  child.on('error', (error) => fail(error.message));
  child.on('exit', (code, signal) => process.exit(code ?? (signal ? 1 : 0)));
} else {
  if (!['build', 'check', 'test', 'clippy', 'run', 'fmt', 'metadata'].includes(args[0]))
    fail('use build/check/test/clippy/run/fmt/metadata/watch; clean is prohibited (it deletes shared intermediates). Use raw cargo for release distribution and other tools.');
  // Scope is this checkout, including calls from crates/db. Runtime arguments are opaque.
  const location = spawnSync('cargo', ['locate-project', '--workspace', '--message-format', 'plain'], { encoding: 'utf8' });
  if (location.status !== 0 || realpathSync(dirname(location.stdout.trim())) !== root)
    fail('invoke this entry only inside its own LVK checkout.');
  const compileArgs = args.slice(0, args.indexOf('--') < 0 ? args.length : args.indexOf('--'));
  if (compileArgs.some((arg) => /^(--config|--manifest-path|--target-dir)(=|$)/.test(arg) || arg.startsWith('+') || arg === '-C' || arg.startsWith('-C')))
    fail('config/manifest/target/toolchain overrides require raw cargo outside this development policy.');
  if (process.env.CARGO_INCREMENTAL !== undefined && process.env.CARGO_INCREMENTAL !== '0')
    fail('CARGO_INCREMENTAL overrides profile.incremental; unset it or set 0 explicitly.');
  for (const profile of ['DEV', 'TEST']) {
    for (const [key, expected] of [['DEBUG', 'line-tables-only'], ['INCREMENTAL', 'false']]) {
      const variable = `CARGO_PROFILE_${profile}_${key}`;
      if (process.env[variable] !== undefined && process.env[variable] !== expected)
        fail(`${variable} conflicts with the development policy; unset it.`);
    }
  }
  for (const variable of ['CARGO_TARGET_DIR', 'CARGO_BUILD_TARGET_DIR', 'CARGO_BUILD_BUILD_DIR'])
    if (process.env[variable] !== undefined) fail(`${variable} conflicts with checkout-local targets/shared build-dir; unset it.`);
  if (/(?:debuginfo|(?:^|\s)-g(?:\s|$))/.test(`${process.env.RUSTFLAGS || ''} ${process.env.CARGO_ENCODED_RUSTFLAGS?.replaceAll('\x1f', ' ') || ''} ${process.env.CARGO_BUILD_RUSTFLAGS || ''}`))
    fail('inherited Rust flags override debug information; remove that override.');
  const config = [];
  const set = (key, value) => config.push('--config', `${key}=${JSON.stringify(value)}`);
  for (const profile of ['dev', 'test']) {
    set(`profile.${profile}.debug`, 'line-tables-only');
    set(`profile.${profile}.incremental`, false);
  }
  set('build.target-dir', join(root, 'target'));
  const shared = join(root, '.lvk-shared/cache');
  if (existsSync(shared)) {
    const build = join(realpathSync(shared), 'lvk-dev-cargo-build');
    mkdirSync(build, { recursive: true });
    set('build.build-dir', build);
  } else {
    // Explicitly override inherited build-dir config even in an ordinary checkout.
    set('build.build-dir', join(root, 'target'));
    warn('no .lvk-shared/cache: intermediates stay in this checkout target/.');
  }
  if (process.env.RUSTC_WRAPPER !== undefined && resolve(process.env.RUSTC_WRAPPER) !== wrapper)
    fail('explicit RUSTC_WRAPPER: preserve it with raw cargo or unset it to opt into shared sccache.');
  const existing = spawnSync('cargo', ['-Z', 'unstable-options', 'config', 'get', 'build.rustc-wrapper', '--format', 'json'], { encoding: 'utf8' });
  if (existing.error) fail(existing.error.message);
  if (existing.status !== 0 && !existing.stderr.includes('is not set')) fail(existing.stderr.trim());
  if (existing.status === 0) {
    const value = JSON.parse(existing.stdout).build?.['rustc-wrapper'];
    if (value && resolve(value) !== wrapper) fail('an explicit build.rustc-wrapper/RUSTC_WRAPPER is configured; preserve it with raw cargo or unset it to opt into shared sccache.');
  }
  const workspaceWrapper = spawnSync('cargo', ['-Z', 'unstable-options', 'config', 'get', 'build.rustc-workspace-wrapper', '--format', 'json'], { encoding: 'utf8' });
  if (workspaceWrapper.status !== 0 && !workspaceWrapper.stderr?.includes('is not set'))
    fail(workspaceWrapper.stderr?.trim() || 'unable to inspect workspace wrapper config');
  if (workspaceWrapper.status === 0 && JSON.parse(workspaceWrapper.stdout).build?.['rustc-workspace-wrapper'])
    fail('explicit configured workspace rustc wrapper: use raw cargo or unset it.');
  if (process.env.RUSTC_WORKSPACE_WRAPPER || process.env.CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER)
    fail('explicit workspace rustc wrapper: use raw cargo or unset it to opt in.');
  if (executable('sccache')) {
    const expected = existsSync(shared) ? join(realpathSync(shared), 'sccache') : process.env.SCCACHE_DIR;
    const stats = spawnSync('sccache', ['--show-stats'], {
      encoding: 'utf8',
      env: expected ? { ...process.env, SCCACHE_DIR: expected } : process.env,
    });
    if (stats.status !== 0) fail(`sccache stats unavailable: ${stats.stderr}`);
    const location = stats.stdout.split('\n').find((line) => line.startsWith('Cache location'));
    if (expected && !location?.includes(`"${expected}"`))
      warn(`shared server cache mismatch: expected ${expected}; ${location || 'location unknown'}. Server is left unchanged.`);
    set('build.rustc-wrapper', wrapper);
  } else warn('sccache unavailable: using rustc directly.');
  const child = spawn('cargo', [...config, ...args], { stdio: 'inherit' });
  for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, () => child.kill(signal));
  child.on('error', (error) => fail(error.message));
  child.on('exit', (code, signal) => process.exit(code ?? (signal ? 1 : 0)));
}
