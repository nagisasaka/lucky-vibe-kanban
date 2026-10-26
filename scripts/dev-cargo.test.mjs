import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, cpSync, symlinkSync, existsSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, delimiter } from 'node:path';
import { spawnSync, spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const scripts = fileURLToPath(new URL('.', import.meta.url));
const cargo = spawnSync('rustup', ['which', 'cargo'], { encoding: 'utf8' }).stdout.trim();
const toolbin = cargo.slice(0, cargo.lastIndexOf('/'));
const run = (cwd, args, env) => spawnSync(process.execPath, [join(cwd, 'scripts/dev-cargo.mjs'), ...args], { cwd, env, encoding: 'utf8', timeout: 30000 });
const ok = (r) => assert.equal(r.status, 0, r.stderr);

test('development policy, shared intermediates, runtime boundary, lock and clean on pinned Cargo', { timeout: 30000 }, async () => {
  const dir = mkdtempSync(join(tmpdir(), 'lvk-dev-cargo-test-'));
  const nodebin = join(dir, 'nodebin'); mkdirSync(nodebin); symlinkSync(process.execPath, join(nodebin, 'node'));
  const env = { ...process.env, PATH: [nodebin, toolbin, '/usr/bin', '/bin'].join(delimiter) };
  for (const key of Object.keys(env)) if (/^(CARGO_|RUSTC_|RUSTFLAGS$|SCCACHE_)/.test(key)) delete env[key];
  try {
    const shared = join(dir, 'cache'); mkdirSync(shared);
    const fixture = (name, share = true) => {
      const cwd = join(dir, name); mkdirSync(join(cwd, 'src'), { recursive: true });
      mkdirSync(join(cwd, 'scripts'));
      for (const file of ['dev-cargo.mjs', 'dev-rustc-cache.mjs']) cpSync(join(scripts, file), join(cwd, 'scripts', file));
      if (share) { mkdirSync(join(cwd, '.lvk-shared')); symlinkSync(shared, join(cwd, '.lvk-shared/cache')); }
      writeFileSync(join(cwd, 'Cargo.toml'), '[package]\nname="probe"\nversion="0.1.0"\nedition="2021"\n[profile.release]\ndebug=1\n');
      writeFileSync(join(cwd, 'src/main.rs'), `fn main() { for key in ["SCCACHE_DIR", "CARGO_BUILD_BUILD_DIR", "CARGO_PROFILE_DEV_DEBUG", "RUSTC_WRAPPER"] { println!("{}={}", key, std::env::var(key).unwrap_or_default()); } if let Some(path) = std::env::args().nth(1) { let child = std::process::Command::new("cargo").args(["-Z", "unstable-options", "config", "get", "build.build-dir"]).current_dir(path).output().unwrap(); println!("child-config={}", child.status.success()); } }`);
      return cwd;
    };
    const a = fixture('a'), b = fixture('b'), plain = fixture('plain', false);
    const first = run(a, ['build', '--offline', '-v'], env); ok(first);
    assert.match(first.stderr, /debuginfo=line-tables-only/); assert.doesNotMatch(first.stderr, /-C incremental=/);
    assert.match(first.stderr, /sccache unavailable/);
    const build = join(shared, 'lvk-dev-cargo-build');
    assert.ok(existsSync(join(build, 'debug/.cargo-lock')));
    ok(run(b, ['build', '--offline'], env));
    for (const cwd of [a, b]) assert.ok(existsSync(join(cwd, 'target/debug/probe')));
    assert.ok(!existsSync(join(a, 'target/debug/deps')));
    const foreign = spawnSync(process.execPath, [join(a, 'scripts/dev-cargo.mjs'), 'check'], { cwd: b, env, encoding: 'utf8' });
    assert.equal(foreign.status, 2); assert.match(foreign.stderr, /own LVK checkout/);
    const release = run(a, ['build', '--release', '--offline', '-v'], env); ok(release);
    assert.match(release.stderr, /debuginfo=1/); assert.doesNotMatch(release.stderr, /line-tables-only/);
    const tested = run(a, ['test', '--offline', '-v'], env); ok(tested);
    assert.match(tested.stderr, /debuginfo=line-tables-only/); assert.doesNotMatch(tested.stderr, /-C incremental=/);
    const local = run(plain, ['build', '--offline'], env); ok(local); assert.match(local.stderr, /intermediates stay/);
    assert.ok(existsSync(join(plain, 'target/debug/deps')));
    for (const vars of [{ CARGO_INCREMENTAL: '1' }, { RUSTC_WRAPPER: '/bin/false' }, { RUSTC_WRAPPER: '' }, { CARGO_BUILD_RUSTC_WRAPPER: '/bin/false' }, { CARGO_PROFILE_DEV_DEBUG: '2' }])
      assert.equal(run(a, ['check'], { ...env, ...vars }).status, 2);
    assert.equal(run(a, ['clean'], env).status, 2);
    assert.equal(run(a, ['check', '--manifest-path', join(b, 'Cargo.toml')], env).status, 2);

    // Fake only sccache transport; real rustc/Cargo still compile the fixture.
    const bin = join(dir, 'bin'); mkdirSync(bin);
    writeFileSync(join(bin, 'sccache'), `#!${process.execPath}\nconst {spawnSync}=require('node:child_process'); const fs=require('node:fs'); if(process.argv[2]==='--show-stats') { console.log('Cache location Local disk: "mismatch"'); } else { fs.appendFileSync(${JSON.stringify(join(dir, 'compiler-env'))}, process.env.SCCACHE_DIR+'\\n'); const r=spawnSync(process.argv[2],process.argv.slice(3),{stdio:'inherit'}); process.exit(r.status??1); }`, { mode: 0o755 });
    const cachedEnv = { ...env, PATH: `${bin}${delimiter}${env.PATH}` };
    const runtime = run(a, ['run', '--offline', '--quiet', '--', plain], cachedEnv); ok(runtime);
    assert.match(runtime.stderr, /cache mismatch/);
    assert.equal(runtime.stdout.trim(), 'SCCACHE_DIR=\nCARGO_BUILD_BUILD_DIR=\nCARGO_PROFILE_DEV_DEBUG=\nRUSTC_WRAPPER=\nchild-config=false');
    assert.ok(readFileSync(join(dir, 'compiler-env'), 'utf8').includes(join(shared, 'sccache')));

    writeFileSync(join(bin, 'cargo'), `#!${process.execPath}\nrequire('node:fs').writeFileSync(${JSON.stringify(join(dir, 'watch-args'))}, JSON.stringify({args:process.argv.slice(2), cache:process.env.SCCACHE_DIR}));`, { mode: 0o755 });
    ok(run(a, ['watch'], cachedEnv));
    const watched = JSON.parse(readFileSync(join(dir, 'watch-args'), 'utf8'));
    assert.equal(watched.args.length, 5);
    assert.deepEqual(watched.args.slice(0, 4), ['watch', '-w', 'crates', '-s']);
    assert.match(watched.args[4], /dev-cargo.mjs.*build --bin server --bin agent-process-host && .*dev-cargo.mjs.*run --bin server/);
    assert.equal(watched.cache, undefined);

    // Cargo owns locking: a sleeping build script holds the shared build lock.
    writeFileSync(join(a, 'build.rs'), 'fn main(){ std::thread::sleep(std::time::Duration::from_secs(2)); }');
    const child = spawn(process.execPath, [join(a, 'scripts/dev-cargo.mjs'), 'build', '--offline'], { cwd: a, env });
    let output = ''; child.stderr.on('data', (chunk) => { output += chunk; });
    const ended = new Promise((resolve) => child.on('exit', resolve));
    try {
      await new Promise((resolve, reject) => { const deadline = Date.now() + 10000; const check = () => output.includes('Compiling') ? resolve() : Date.now() > deadline ? reject(new Error(output)) : setTimeout(check, 10); check(); });
      const other = run(b, ['build', '--offline'], env); ok(other);
      assert.match(other.stderr, /Blocking waiting for file lock on build directory/);
    } finally { assert.equal(await ended, 0); }
    // Raw clean only in disposable fixture: proves shared intermediates are removed.
    const clean = spawnSync(cargo, ['--config', `build.build-dir=${JSON.stringify(build)}`, 'clean'], { cwd: a, env, encoding: 'utf8' }); ok(clean);
    assert.ok(!existsSync(build)); assert.ok(existsSync(join(b, 'target/debug/probe')));
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('optional real installed sccache smoke (no server reconfiguration)', { skip: process.env.LVK_TEST_REAL_SCCACHE !== '1', timeout: 30000 }, () => {
  const dir = mkdtempSync(join(tmpdir(), 'lvk-real-sccache-'));
  const env = { ...process.env };
  for (const key of Object.keys(env)) if (/^(CARGO_|RUSTC_|RUSTFLAGS$|SCCACHE_DIR$)/.test(key)) delete env[key];
  try {
    mkdirSync(join(dir, 'scripts')); mkdirSync(join(dir, 'src'));
    for (const file of ['dev-cargo.mjs', 'dev-rustc-cache.mjs']) cpSync(join(scripts, file), join(dir, 'scripts', file));
    writeFileSync(join(dir, 'Cargo.toml'), '[package]\nname="lvk_cache_smoke"\nversion="0.1.0"\nedition="2021"\n');
    writeFileSync(join(dir, 'src/lib.rs'), 'pub fn value() -> u8 { 42 }\n');
    const result = run(dir, ['build', '--offline', '-v'], env); ok(result);
    assert.doesNotMatch(result.stderr, /sccache unavailable/);
    assert.match(result.stderr, /Running `.*dev-rustc-cache.mjs/);
    assert.match(result.stderr, /debuginfo=line-tables-only/);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});
