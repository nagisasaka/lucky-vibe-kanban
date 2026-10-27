#!/usr/bin/env node
// LVK contributor entry point. Never export compiler settings into the server.
import { existsSync, realpathSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync, spawn } from 'node:child_process';

const root = realpathSync(join(dirname(fileURLToPath(import.meta.url)), '..'));
const args = process.argv.slice(2);
let purpose = 'preview';
if (args[0] === '--purpose') { args.shift(); purpose = args.shift(); }
const fail = (message) => { console.error(`dev-cargo: ${message}`); process.exit(2); };
const warn = (message) => console.error(`dev-cargo: ${message}`);
const quote = (value) => `'${value.replaceAll("'", "'\\''")}'`;
if (!['preview', 'dev', 'stable'].includes(purpose)) fail('purpose must be preview/dev/stable.');
const release = purpose !== 'preview';
if (args[0] === 'watch') {
  if (args.length !== 1) fail('watch accepts no arguments; use backend:dev:watch.');
  const entry = `${quote(process.execPath)} ${quote(fileURLToPath(import.meta.url))} --purpose ${purpose}`;
  const child = spawn('cargo', ['watch', '-w', 'crates', '-s',
    `${entry} build --bin server --bin agent-process-host && ${entry} run --bin server`],
  { cwd: root, stdio: 'inherit' });
  for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, () => child.kill(signal));
  child.on('error', (error) => fail(error.message));
  child.on('exit', (code, signal) => process.exit(code ?? (signal ? 1 : 0)));
} else {
  if (!['build', 'check', 'test', 'clippy', 'run', 'fmt', 'metadata'].includes(args[0]))
    fail('use build/check/test/clippy/run/fmt/metadata/watch; clean is prohibited (it deletes shared intermediates). Select --purpose dev for the optimised development server.');
  // Scope is this checkout, including calls from crates/db. Runtime arguments are opaque.
  const location = spawnSync('cargo', ['locate-project', '--workspace', '--message-format', 'plain'], { encoding: 'utf8' });
  if (location.status !== 0 || realpathSync(dirname(location.stdout.trim())) !== root)
    fail('invoke this entry only inside its own LVK checkout.');
  const compileArgs = args.slice(0, args.indexOf('--') < 0 ? args.length : args.indexOf('--'));
  if (compileArgs.some((arg) => /^(--config|--manifest-path|--target-dir)(=|$)/.test(arg) || arg.startsWith('+') || arg === '-C' || arg.startsWith('-C')))
    fail('config/manifest/target/toolchain overrides require raw cargo outside this development policy.');
  if (compileArgs.some((arg) => /^(--release|--profile)(=|$)/.test(arg) || arg === '-r'))
    fail('select --purpose preview/dev/stable instead of release/profile overrides.');
  if (process.env.CARGO_INCREMENTAL !== undefined)
    fail('CARGO_INCREMENTAL overrides both dev/test and release profile.incremental; unset it to keep dev/test=true and release=false.');
  if (process.env.CARGO_BUILD_INCREMENTAL !== undefined)
    fail('CARGO_BUILD_INCREMENTAL overrides dev/test and release profile.incremental; unset it.');
  const buildIncremental = spawnSync('cargo', ['-Z', 'unstable-options', 'config', 'get', 'build.incremental', '--format', 'json'], { encoding: 'utf8' });
  if (buildIncremental.status !== 0 && !buildIncremental.stderr?.includes('is not set'))
    fail('unable to inspect build.incremental configuration.');
  if (buildIncremental.status === 0 && JSON.parse(buildIncremental.stdout).build?.incremental !== undefined)
    fail('configured build.incremental overrides dev/test and release profile.incremental; remove it or use raw cargo deliberately.');
  const profiles = Object.fromEntries(['dev', 'test', 'release'].map((profile) => [profile, {
    debug: purpose === 'stable' ? 0 : 'line-tables-only',
    incremental: profile !== 'release',
    'opt-level': profile === 'release' ? 3 : 0,
    strip: purpose === 'stable' ? 'symbols' : 'none',
    'split-debuginfo': 'off',
  }]));
  for (const [profile, values] of Object.entries(profiles)) {
    for (const [key, expected] of Object.entries(values)) {
      const variable = `CARGO_PROFILE_${profile.toUpperCase()}_${key.toUpperCase().replaceAll('-', '_')}`;
      if (process.env[variable] !== undefined && process.env[variable] !== String(expected))
        fail(`${variable} conflicts with ${purpose} policy; unset it.`);
    }
  }
  for (const variable of ['CARGO_TARGET_DIR', 'CARGO_BUILD_TARGET_DIR', 'CARGO_BUILD_BUILD_DIR'])
    if (process.env[variable] !== undefined) fail(`${variable} conflicts with checkout-local targets/shared build-dir; unset it.`);
  const conflictingFlags = /(?:debuginfo|strip|split-debuginfo|opt-level|incremental|(?:^|\s)-[gO](?:\s|$))/;
  const flagValues = Object.entries(process.env)
    .filter(([key]) => /^(RUSTFLAGS|CARGO_ENCODED_RUSTFLAGS|CARGO_BUILD_RUSTFLAGS|CARGO_TARGET_.*_RUSTFLAGS)$/.test(key))
    .map(([, value]) => value.replaceAll('\x1f', ' '));
  // Configured build/target rustflags also override profile codegen settings.
  for (const key of ['build.rustflags', 'target']) {
    const found = spawnSync('cargo', ['-Z', 'unstable-options', 'config', 'get', key, '--format', 'json'], { encoding: 'utf8' });
    if (found.status !== 0 && !found.stderr?.includes('is not set'))
      fail('unable to inspect configured Rust flags.');
    if (found.status === 0) {
      const collect = (value, name = '') => {
        if (name === 'rustflags') flagValues.push(Array.isArray(value) ? value.join(' ') : String(value));
        else if (value && typeof value === 'object')
          for (const [child, item] of Object.entries(value)) collect(item, child);
      };
      collect(JSON.parse(found.stdout));
    }
  }
  if (flagValues.some((value) => conflictingFlags.test(value)))
    fail('inherited Rust flags override opt/debug/strip/split/incremental policy; remove that override.');
  const config = [];
  const set = (key, value) => config.push('--config', `${key}=${JSON.stringify(value)}`);
  for (const [profile, values] of Object.entries(profiles))
    for (const [key, value] of Object.entries(values)) set(`profile.${profile}.${key}`, value);
  if (release && !['fmt', 'metadata'].includes(args[0])) args.splice(1, 0, '--release');
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
  if (process.env.RUSTC_WRAPPER !== undefined)
    fail('explicit RUSTC_WRAPPER: preserve it with raw cargo or unset it to use this entry without a compiler wrapper.');
  const existing = spawnSync('cargo', ['-Z', 'unstable-options', 'config', 'get', 'build.rustc-wrapper', '--format', 'json'], { encoding: 'utf8' });
  if (existing.error) fail(existing.error.message);
  if (existing.status !== 0 && !existing.stderr.includes('is not set')) fail(existing.stderr.trim());
  if (existing.status === 0) {
    const value = JSON.parse(existing.stdout).build?.['rustc-wrapper'];
    if (value) fail('an explicit build.rustc-wrapper/RUSTC_WRAPPER is configured; preserve it with raw cargo or unset it to use this entry without a compiler wrapper.');
  }
  const workspaceWrapper = spawnSync('cargo', ['-Z', 'unstable-options', 'config', 'get', 'build.rustc-workspace-wrapper', '--format', 'json'], { encoding: 'utf8' });
  if (workspaceWrapper.status !== 0 && !workspaceWrapper.stderr?.includes('is not set'))
    fail(workspaceWrapper.stderr?.trim() || 'unable to inspect workspace wrapper config');
  if (workspaceWrapper.status === 0 && JSON.parse(workspaceWrapper.stdout).build?.['rustc-workspace-wrapper'])
    fail('explicit configured workspace rustc wrapper: use raw cargo or unset it.');
  if (process.env.RUSTC_WORKSPACE_WRAPPER || process.env.CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER)
    fail('explicit workspace rustc wrapper: use raw cargo or unset it to opt in.');
  const child = spawn('cargo', [...config, ...args], { stdio: 'inherit' });
  for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, () => child.kill(signal));
  child.on('error', (error) => fail(error.message));
  child.on('exit', (code, signal) => process.exit(code ?? (signal ? 1 : 0)));
}
