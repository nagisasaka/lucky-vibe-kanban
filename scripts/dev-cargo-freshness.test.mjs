import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, cpSync, symlinkSync, utimesSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const repo = fileURLToPath(new URL('..', import.meta.url));
const ok = (r) => { assert.equal(r.status, 0, r.stderr); return r.stdout.trim(); };
const command = (cmd, args, cwd, env) => spawnSync(cmd, args, { cwd, env, encoding: 'utf8', timeout: 30000 });

test('official entry checks content across A/B/A and refreshes server build inputs and runtime build root', { timeout: 60000 }, () => {
  const dir = mkdtempSync(join(tmpdir(), 'lvk-cargo-freshness-'));
  const env = { ...process.env };
  for (const key of Object.keys(env)) if (/^(CARGO_|RUSTC_|RUSTFLAGS$|SCCACHE_|LVK_BUILD_)/.test(key)) delete env[key];
  for (const key of ['POSTHOG_API_KEY', 'POSTHOG_API_ENDPOINT', 'SENTRY_DSN']) delete env[key];
  // Isolate Cargo config and registry state, but keep the installed pinned toolchain.
  env.CARGO_HOME = join(dir, 'cargo-home');
  for (const role of ['AUTHOR', 'COMMITTER']) {
    const ident = ok(command('git', ['var', `GIT_${role}_IDENT`], repo, process.env));
    const [, name, email] = ident.match(/^(.*) <([^>]+)> /);
    env[`GIT_${role}_NAME`] = name; env[`GIT_${role}_EMAIL`] = email;
  }
  const old = new Date('2020-01-01T00:00:00Z');
  const write = (p, s) => { writeFileSync(p, s); utimesSync(p, old, old); };
  try {
    mkdirSync(join(dir, 'cache'));
    const roots = ['a', 'b'].map((name) => {
      const root = join(dir, name);
      for (const p of ['scripts', '.lvk-shared', 'src', 'crates/server/src']) mkdirSync(join(root, p), { recursive: true });
      symlinkSync(join(dir, 'cache'), join(root, '.lvk-shared/cache'));
      cpSync(join(repo, 'rust-toolchain.toml'), join(root, 'rust-toolchain.toml'));
      cpSync(join(repo, 'scripts/dev-cargo.mjs'), join(root, 'scripts/dev-cargo.mjs'));
      write(join(root, 'Cargo.toml'), '[workspace]\nmembers=["crates/server"]\n[package]\nname="api-probe"\nversion="0.1.0"\nedition="2021"\n');
      const value = name === 'a' ? '111' : '222';
      write(join(root, 'src/main.rs'), `fn main(){println!("${value}");}`);
      write(join(root, 'crates/server/Cargo.toml'), '[package]\nname="server"\nversion="0.1.0"\nedition="2021"\n');
      cpSync(join(repo, 'crates/server/build_metadata.rs'), join(root, 'crates/server/build_metadata.rs'));
      // Only the dotenv dependency is shimmed; build.rs and metadata resolver are real source.
      const dotenv = `mod dotenv { pub fn from_path(p: &std::path::Path) -> std::io::Result<()> {
        println!("cargo:rustc-env=PROBE_ENV_ROOT={}",p.parent().unwrap().canonicalize()?.display());
        let text=std::fs::read_to_string(p)?;
        for line in text.lines(){if let Some((k,v))=line.split_once('='){std::env::set_var(k,v);}} Ok(())
      }}\n`;
      write(join(root, 'crates/server/build.rs'), dotenv + readFileSync(join(repo, 'crates/server/build.rs'), 'utf8'));
      write(join(root, 'crates/server/src/main.rs'), 'fn main(){println!("{}|{}|{}",env!("LVK_BUILD_REVISION"),env!("PROBE_ENV_ROOT"),option_env!("POSTHOG_API_KEY").unwrap_or("none"));}');
      write(join(root, '.env'), 'POSTHOG_API_KEY=AAA\n');
      writeFileSync(join(root, '.gitignore'), 'target/\n.lvk-shared/\npackages/\nCargo.lock\n');
      ok(command('git', ['init', '-q'], root, env));
      ok(command('git', ['add', '.'], root, env));
      ok(command('git', ['commit', '-qm', name], root, env));
      return root;
    });
    const build = (root, pkg, extra = {}) => {
      const result = command(process.execPath, [join(root, 'scripts/dev-cargo.mjs'), 'build', '--offline', '-p', pkg, '-v'], root, { ...env, ...extra });
      ok(result); return result.stderr;
    };
    const run = (root, name) => ok(command(join(root, 'target/debug', name), [], dir, env));
    for (const root of [roots[0], roots[1], roots[0]]) {
      build(root, 'api-probe');
      assert.equal(run(root, 'api-probe'), root === roots[0] ? '111' : '222');
      const log = build(root, 'server');
      if (root === roots[1]) assert.doesNotMatch(log, /--crate-name build_script_build/);
      const sha = ok(command('git', ['rev-parse', 'HEAD'], root, env));
      assert.equal(run(root, 'server'), `${sha}|${root}|AAA`);
    }
    // Same cwd, equal-size content edit, same old mtime: checksum must invalidate.
    write(join(roots[0], 'src/main.rs'), 'fn main(){println!("333");}');
    build(roots[0], 'api-probe'); assert.equal(run(roots[0], 'api-probe'), '333');
    mkdirSync(join(roots[0], '.cargo'));
    writeFileSync(join(roots[0], '.cargo/config.toml'), '[unstable]\nchecksum-freshness=false\n');
    write(join(roots[0], 'src/main.rs'), 'fn main(){println!("444");}');
    const configured = build(roots[0], 'api-probe');
    assert.match(configured, /checksum-hash-algorithm/);
    assert.equal(run(roots[0], 'api-probe'), '444');
    // Explicit identity formerly bypassed the missing sentinel. Preserve input mtime.
    const injected = { LVK_BUILD_REVISION: 'a'.repeat(40) };
    build(roots[0], 'server', injected);
    write(join(roots[0], '.env'), 'POSTHOG_API_KEY=BBB\n');
    build(roots[0], 'server', injected);
    assert.equal(run(roots[0], 'server'), `${'a'.repeat(40)}|${roots[0]}|BBB`);
    rmSync(join(roots[0], '.env'));
    build(roots[0], 'server', injected);
    assert.equal(run(roots[0], 'server'), `${'a'.repeat(40)}|${roots[0]}|none`);
    const conflict = command(process.execPath, [join(roots[0], 'scripts/dev-cargo.mjs'), 'check'], roots[0], { ...env, CARGO_UNSTABLE_CHECKSUM_FRESHNESS: 'false' });
    assert.equal(conflict.status, 2); assert.match(conflict.stderr, /checksum-freshness/);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});
