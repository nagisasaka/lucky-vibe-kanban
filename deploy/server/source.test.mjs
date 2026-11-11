import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import {
  mkdtempSync,
  mkdirSync,
  readFileSync,
  writeFileSync,
  rmSync,
  existsSync,
  cpSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

const script = fileURLToPath(new URL("bundle-source.sh", import.meta.url));
function fixture(run) {
  const dir = mkdtempSync(join(tmpdir(), "lvk-source-test-"));
  const upstream = join(dir, "public");
  const git = (cwd, ...args) =>
    execFileSync("git", ["-C", cwd, ...args], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    }).trim();
  try {
    mkdirSync(upstream);
    git(upstream, "init", "--template=");
    git(upstream, "config", "user.name", "Public source fixture");
    git(upstream, "config", "user.email", "fixture@example.invalid");
    const commits = [];
    for (const value of ["first", "release", "future"]) {
      writeFileSync(join(upstream, "README.md"), `${value}\n`);
      git(upstream, "add", "README.md");
      git(upstream, "commit", "-m", `Public ${value} intent`);
      commits.push(git(upstream, "rev-parse", "HEAD"));
    }
    run({ dir, upstream, git, commits });
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

test("bundle uses exact public commit and full ancestors with shallow/no-git build input", () =>
  fixture(({ dir, upstream, git, commits }) => {
    const shallow = join(dir, "ci");
    execFileSync("git", ["clone", "--depth=1", `file://${upstream}`, shallow], {
      stdio: "pipe",
    });
    assert.equal(git(shallow, "rev-parse", "--is-shallow-repository"), "true");
    // The build-context payload contains only the helper, no caller gitdir.
    const context = join(dir, "context");
    mkdirSync(context);
    cpSync(script, join(context, "bundle-source.sh"));
    const hostileConfig = join(dir, "builder.gitconfig");
    writeFileSync(
      hostileConfig,
      `[url "file:///nonexistent-builder-repository"]\n  insteadOf = file://${upstream}\n[credential]\n  helper = builder-only-secret-helper\n`,
    );
    const output = join(dir, "source");
    execFileSync(
      "sh",
      [
        join(context, "bundle-source.sh"),
        `file://${upstream}`,
        commits[1],
        output,
      ],
      {
        cwd: shallow,
        stdio: "pipe",
        env: {
          ...process.env,
          GIT_CONFIG_GLOBAL: hostileConfig,
          GIT_ALTERNATE_OBJECT_DIRECTORIES: "/nonexistent-builder-objects",
        },
      },
    );
    assert.equal(git(output, "rev-parse", "HEAD"), commits[1]);
    assert.equal(readFileSync(join(output, "README.md"), "utf8"), "release\n");
    assert.equal(git(output, "rev-parse", "--is-shallow-repository"), "false");
    assert.equal(git(output, "rev-list", "--count", "HEAD"), "2");
    assert.equal(git(output, "for-each-ref"), "");
    assert(!existsSync(join(output, ".git/objects/info/alternates")));
    assert(!existsSync(join(output, ".git/hooks")));
    assert(!readFileSync(join(output, ".git/config"), "utf8").includes("user"));
    // Removing every source proves the output is self-contained.
    rmSync(upstream, { recursive: true });
    rmSync(shallow, { recursive: true });
    assert.match(git(output, "log", "--format=%s"), /Public first intent/);
    assert.match(git(output, "show", commits[1]), /-first\n\+release/);
    git(output, "fsck", "--full", "--strict");
  }));

test("unavailable, non-full and mismatched revisions fail instead of using upstream HEAD", () =>
  fixture(({ dir, upstream, commits }) => {
    for (const [index, url, revision] of [
      [0, `file://${upstream}`, "HEAD"],
      [1, `file://${upstream}`, "a".repeat(40)],
      [2, `file://${join(dir, "absent")}`, commits[1]],
    ]) {
      const result = spawnSync(
        "sh",
        [script, url, revision, join(dir, `bad-${index}`)],
        { encoding: "utf8" },
      );
      assert.notEqual(result.status, 0, result.stdout + result.stderr);
    }
  }));

test("Docker fetch stage is independent of CI Git and owns source outside persistent volumes", () => {
  const docker = readFileSync(
    new URL("../../Dockerfile", import.meta.url),
    "utf8",
  );
  assert.match(
    docker,
    /FROM rust-toolchain AS bundled-source\nARG LVK_BUILD_REVISION=""/,
  );
  assert.match(
    docker,
    /https:\/\/github.com\/nagisasaka\/lucky-vibe-kanban.git \\\n    "\$LVK_BUILD_REVISION" \/opt\/lvk\/source/,
  );
  assert.match(
    docker,
    /COPY --from=bundled-source --chown=10001:10001 \/opt\/lvk\/source \/opt\/lvk\/source/,
  );
  assert.match(
    readFileSync(new URL("../../.dockerignore", import.meta.url), "utf8"),
    /^\.git\/$/m,
  );
  const compose = readFileSync(
    new URL("compose.yaml", import.meta.url),
    "utf8",
  );
  assert(!compose.includes("/opt/lvk/source"));
});
