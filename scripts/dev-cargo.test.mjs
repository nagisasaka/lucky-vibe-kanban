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
      cpSync(join(scripts, 'dev-cargo.mjs'), join(cwd, 'scripts/dev-cargo.mjs'));
      if (share) { mkdirSync(join(cwd, '.lvk-shared')); symlinkSync(shared, join(cwd, '.lvk-shared/cache')); }
      writeFileSync(join(cwd, 'Cargo.toml'), '[package]\nname="probe"\nversion="0.1.0"\nedition="2021"\n[profile.release]\ndebug=1\nstrip=true\nsplit-debuginfo="packed"\n');
      writeFileSync(join(cwd, 'src/main.rs'), `fn main() { for key in ["SCCACHE_DIR", "CARGO_BUILD_BUILD_DIR", "CARGO_PROFILE_DEV_DEBUG", "CARGO_INCREMENTAL", "RUSTC_WRAPPER"] { println!("{}={}", key, std::env::var(key).unwrap_or_default()); } if let Some(path) = std::env::args().nth(1) { let child = std::process::Command::new("cargo").args(["-Z", "unstable-options", "config", "get", "build.build-dir"]).current_dir(path).output().unwrap(); println!("child-config={}", child.status.success()); } }`);
      return cwd;
    };
    const a = fixture('a'), b = fixture('b'), plain = fixture('plain', false);
    const first = run(a, ['build', '--offline', '-v'], env); ok(first);
    assert.match(first.stderr, /debuginfo=line-tables-only/); assert.match(first.stderr, /-C incremental=/);
    assert.doesNotMatch(first.stderr, /opt-level=|strip=|split-debuginfo=packed/);
    assert.doesNotMatch(first.stderr, /sccache|dev-rustc-cache/);
    const build = join(shared, 'lvk-dev-cargo-build');
    assert.ok(existsSync(join(build, 'debug/.cargo-lock')));
    assert.ok(existsSync(join(build, 'debug/incremental')));
    ok(run(b, ['build', '--offline'], env));
    for (const cwd of [a, b]) assert.ok(existsSync(join(cwd, 'target/debug/probe')));
    assert.ok(!existsSync(join(a, 'target/debug/deps')));
    const foreign = spawnSync(process.execPath, [join(a, 'scripts/dev-cargo.mjs'), 'check'], { cwd: b, env, encoding: 'utf8' });
    assert.equal(foreign.status, 2); assert.match(foreign.stderr, /own LVK checkout/);
    const lines = (path) => spawnSync('readelf', ['--debug-dump=decodedline', path], { encoding: 'utf8' }).stdout;
    assert.match(lines(join(a, 'target/debug/probe')), /main.rs/);
    for (const purpose of ['dev', 'stable']) {
      const release = run(a, ['--purpose', purpose, 'build', '--offline', '-v'], env); ok(release);
      assert.match(release.stderr, /opt-level=3/);
      assert.doesNotMatch(release.stderr, /-C incremental=/);
      if (purpose === 'dev') {
        assert.match(release.stderr, /debuginfo=line-tables-only/);
        assert.doesNotMatch(release.stderr, /strip=|split-debuginfo=packed/);
        const placed = join(dir, 'packaged-dev');
        cpSync(join(a, 'target/release/probe'), placed);
        assert.match(lines(placed), /main.rs/);
      } else {
        assert.doesNotMatch(release.stderr, /debuginfo=(?:1|2|line-tables-only)/);
        assert.match(release.stderr, /strip=symbols/);
        assert.doesNotMatch(lines(join(a, 'target/release/probe')), /main.rs/);
      }
    }
    assert.equal(run(a, ['build', '--release'], env).status, 2);
    // Exercise the exact command-local stable profile used by both Docker stages.
    const docker = readFileSync(join(scripts, '../Dockerfile'), 'utf8');
    const prefixes = [...docker.matchAll(/RUN (CARGO_PROFILE_RELEASE_DEBUG=0[\s\S]*?) cargo (?:chef cook|build) --locked --release/g)];
    assert.equal(prefixes.length, 2);
    assert.equal(prefixes[0][1], prefixes[1][1]);
    const stableEnv = { ...env };
    for (const match of prefixes[0][1].matchAll(/(CARGO_PROFILE_\w+)=(\w+)/g)) stableEnv[match[1]] = match[2];
    ok(spawnSync(cargo, ['build', '--release', '--offline', '-v'], { cwd: plain, env: stableEnv, encoding: 'utf8' }));
    const packaged = join(dir, 'packaged-stable');
    cpSync(join(plain, 'target/release/probe'), packaged);
    assert.doesNotMatch(lines(packaged), /main.rs/);

    const tested = run(a, ['test', '--offline', '-v'], env); ok(tested);
    assert.match(tested.stderr, /debuginfo=line-tables-only/); assert.match(tested.stderr, /-C incremental=/);
    const local = run(plain, ['build', '--offline'], env); ok(local); assert.match(local.stderr, /intermediates stay/);
    assert.ok(existsSync(join(plain, 'target/debug/deps')));
    for (const vars of [{ CARGO_INCREMENTAL: '1' }, { CARGO_INCREMENTAL: '0' }, { CARGO_PROFILE_DEV_INCREMENTAL: 'false' }, { CARGO_PROFILE_TEST_INCREMENTAL: 'false' }, { CARGO_PROFILE_RELEASE_INCREMENTAL: 'true' }, { RUSTC_WRAPPER: '/bin/false' }, { RUSTC_WRAPPER: '' }, { CARGO_BUILD_RUSTC_WRAPPER: '/bin/false' }, { CARGO_PROFILE_DEV_DEBUG: '2' }, { CARGO_PROFILE_DEV_OPT_LEVEL: '1' }, { CARGO_PROFILE_RELEASE_STRIP: 'true' }, { CARGO_PROFILE_RELEASE_SPLIT_DEBUGINFO: 'packed' }, { RUSTFLAGS: '-Copt-level=2' }, { CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS: '-Cstrip=symbols' }])
      assert.equal(run(a, ['check'], { ...env, ...vars }).status, 2);
    for (const value of ['0', '1']) {
      for (const purpose of ['preview', 'dev', 'stable']) {
        const conflict = run(a, ['--purpose', purpose, 'check'], { ...env, CARGO_INCREMENTAL: value });
        assert.equal(conflict.status, 2); assert.match(conflict.stderr, /CARGO_INCREMENTAL.*overrides/);
      }
    }
    for (const value of ['true', 'false']) {
      for (const purpose of ['preview', 'dev', 'stable']) {
        const conflict = run(a, ['--purpose', purpose, 'check'], { ...env, CARGO_BUILD_INCREMENTAL: value });
        assert.equal(conflict.status, 2); assert.match(conflict.stderr, /CARGO_BUILD_INCREMENTAL.*overrides/);
      }
    }
    mkdirSync(join(a, '.cargo'));
    for (const value of ['true', 'false']) {
      const contents = `[build]\nincremental=${value}\n`;
      writeFileSync(join(a, '.cargo/config.toml'), contents);
      for (const purpose of ['preview', 'dev', 'stable']) {
        const conflict = run(a, ['--purpose', purpose, 'check'], env);
        assert.equal(conflict.status, 2); assert.match(conflict.stderr, /configured build.incremental.*overrides/);
        assert.equal(readFileSync(join(a, '.cargo/config.toml'), 'utf8'), contents);
      }
    }

    writeFileSync(join(a, '.cargo/config.toml'), '[build]\nrustflags=["-C", "strip=symbols"]\n');
    assert.equal(run(a, ['check'], env).status, 2);
    const wrapperCalls = join(dir, 'wrapper-called');
    const explicitWrapper = join(dir, 'explicit-wrapper');
    writeFileSync(explicitWrapper, `#!${process.execPath}\nrequire('node:fs').writeFileSync(${JSON.stringify(wrapperCalls)}, 'called'); process.exit(98);`, { mode: 0o755 });
    for (const key of ['rustc-wrapper', 'rustc-workspace-wrapper']) {
      writeFileSync(join(a, '.cargo/config.toml'), `[build]\n${key}=${JSON.stringify(explicitWrapper)}\n`);
      const conflict = run(a, ['check'], env);
      assert.equal(conflict.status, 2); assert.match(conflict.stderr, /wrapper/);
      assert.ok(!existsSync(wrapperCalls));
      assert.ok(readFileSync(join(a, '.cargo/config.toml'), 'utf8').includes(explicitWrapper));
    }
    rmSync(join(a, '.cargo'), { recursive: true });
    assert.equal(run(a, ['clean'], env).status, 2);
    assert.equal(run(a, ['check', '--manifest-path', join(b, 'Cargo.toml')], env).status, 2);

    // A poisoned executable catches even stats/probe calls; never contact a daemon.
    const bin = join(dir, 'bin'); mkdirSync(bin);
    const cacheCalls = join(dir, 'sccache-called');
    writeFileSync(join(bin, 'sccache'), `#!${process.execPath}\nrequire('node:fs').writeFileSync(${JSON.stringify(cacheCalls)}, 'called'); process.exit(99);`, { mode: 0o755 });
    const cachedEnv = { ...env, PATH: `${bin}${delimiter}${env.PATH}` };
    // Force new real compilation with sccache installed on PATH.
    ok(run(a, ['check', '--offline'], cachedEnv));
    const runtime = run(a, ['run', '--offline', '--quiet', '--', plain], cachedEnv); ok(runtime);
    assert.equal(runtime.stdout.trim(), 'SCCACHE_DIR=\nCARGO_BUILD_BUILD_DIR=\nCARGO_PROFILE_DEV_DEBUG=\nCARGO_INCREMENTAL=\nRUSTC_WRAPPER=\nchild-config=false');
    const devRuntime = run(a, ['--purpose', 'dev', 'run', '--offline', '--quiet', '--', plain], cachedEnv); ok(devRuntime);
    assert.equal(devRuntime.stdout, runtime.stdout);
    assert.ok(!existsSync(cacheCalls));

    writeFileSync(join(bin, 'cargo'), `#!${process.execPath}\nrequire('node:fs').writeFileSync(${JSON.stringify(join(dir, 'watch-args'))}, JSON.stringify({args:process.argv.slice(2), cache:process.env.SCCACHE_DIR}));`, { mode: 0o755 });
    ok(run(a, ['watch'], cachedEnv));
    const watched = JSON.parse(readFileSync(join(dir, 'watch-args'), 'utf8'));
    assert.equal(watched.args.length, 5);
    assert.deepEqual(watched.args.slice(0, 4), ['watch', '-w', 'crates', '-s']);
    assert.match(watched.args[4], /dev-cargo.mjs.*--purpose preview build --bin server --bin agent-process-host && .*dev-cargo.mjs.*run --bin server/);
    assert.equal(watched.cache, undefined);
    assert.ok(!existsSync(cacheCalls));

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

test('specialised validation runner does not auto-configure a compiler cache or replace explicit wrappers', () => {
  const dir = mkdtempSync(join(tmpdir(), 'lvk-no-compiler-cache-'));
  try {
    const fixture = join(dir, 'tests/integration-validation-settings');
    mkdirSync(fixture, { recursive: true });
    const runner = join(fixture, 'run-rust.py');
    cpSync(join(scripts, '../tests/integration-validation-settings/run-rust.py'), runner);
    const called = join(dir, 'sccache-called');
    writeFileSync(join(dir, 'sccache'), `#!${process.execPath}\nrequire('node:fs').writeFileSync(${JSON.stringify(called)}, 'called'); process.exit(99);`, { mode: 0o755 });
    // Reproduce the old shared-cache discovery precondition without real shared data.
    mkdirSync(join(dir, '.lvk-shared/cache'), { recursive: true });
    const env = { ...process.env, PATH: `${dir}${delimiter}${process.env.PATH}` };
    for (const key of Object.keys(env)) if (/^(RUSTC_WRAPPER|SCCACHE_)/.test(key)) delete env[key];
    const inspect = (extra) => {
      const result = spawnSync('python3', ['-c', 'import runpy,json,sys; e=runpy.run_path(sys.argv[1])["ENV"]; print(json.dumps({k:e[k] for k in ("RUSTC_WRAPPER","SCCACHE_DIR") if k in e}))', runner], { env: { ...env, ...extra }, encoding: 'utf8' });
      ok(result); return JSON.parse(result.stdout);
    };
    assert.deepEqual(inspect({}), {});
    assert.deepEqual(inspect({ RUSTC_WRAPPER: '/explicit/wrapper' }), { RUSTC_WRAPPER: '/explicit/wrapper' });
    assert.ok(!existsSync(called));
  } finally { rmSync(dir, { recursive: true, force: true }); }
});
