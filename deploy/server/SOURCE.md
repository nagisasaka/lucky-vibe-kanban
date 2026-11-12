# Bundled source for version-matched help

Both Docker runtime targets contain `/opt/lvk/source`, outside `/repos`, HOME
and work volumes. `LVK_BUILD_REVISION` must be a full public commit SHA. The
`bundled-source` stage fetches that SHA from
`https://github.com/nagisasaka/lucky-vibe-kanban.git` without a depth limit,
checks out detached HEAD, and fails if retrieval or full-object verification
fails. Fork distributors must change this public upstream to their public repo.
No startup fetch/pull occurs. The same revision argument builds the binary.
A working tree change cannot update the running binary.

The source includes the distribution commit's complete reachable ancestor
history and trees/blobs. It does not intentionally fetch other branches or tags;
there are no branch/tag refs. The new Git directory has no copied caller config,
hooks, credentials, shallow boundary, worktree gitdir, or alternates. Docker's
`.git` exclusion and CI's default shallow checkout remain in effect; neither is
used to provide history. Immediately before the image build, CI checks that
checkout HEAD equals `GITHUB_SHA` and that tracked files have no differences
from HEAD, including staged changes. This detects source changes introduced by
dependency installation or validation before combining the context-built binary
with the independently fetched reference revision. Manual Docker builds must
supply a revision matching their clean context; the helper cannot inspect files
excluded from its context. Only tracked public source is fetched, rather than
copying the build context or the developer's Git directory. Missing published
commits intentionally make builds fail, including manual builds of unpublished
work. No branch HEAD fallback is allowed.

COPY assigns source ownership to runtime UID/GID `10001:10001` (appuser), covering
both server/libgit2 and agent Git CLI without safe.directory configuration. No
wildcard trust or ownership-check disablement is added. Mounted HOME does not
hide this ownership. Public source remains a reference checkout; help requests
must not implement, build, restart, clone or create development Projects without
a separate user request. REVIEW retains its existing general permissions.

`node --test deploy/server/source.test.mjs` uses a disposable public fixture,
a shallow CI clone and a context with no `.git`, verifies the older requested
commit, its ancestor message/diff and no branch/tag refs, then removes the input
repositories and reads history again. Invalid/unavailable revisions must fail.
This is a Docker-free test, not proof of a final distribution image.

Public image CI publishes after a successful build without post-build smoke,
fresh-install or formatting checks. It does not verify image startup,
authentication boundaries or retained-volume upgrades before publication.
The host updater still uses `deploy/developer/verify-image.sh` and
`verify-access.mjs` for its separate candidate verification; those scripts are
not invoked by public image CI. Actual image, ownership/libgit2 and retained-volume
upgrade checks must be reported separately when performed.

No history size cap is imposed. To measure a built image as its runtime user:

```sh
docker run --rm --entrypoint sh IMAGE -ec '
  git -C /opt/lvk/source rev-list --count HEAD
  git -C /opt/lvk/source count-objects -vH
  du -sh /opt/lvk/source/.git /opt/lvk/source
  git -C /opt/lvk/source for-each-ref
'
```

Record the immutable image digest, binary revision, ancestor count and both
sizes. Measure complete image layer/download size separately; filesystem size
alone is not compressed registry transfer size. Optimization is future work.

On 2026-11-11, the helper was independently exercised as UID/GID 10001 against
the anonymous public upstream, with an empty temporary build-input directory:

```sh
sh /absolute/path/to/deploy/server/bundle-source.sh \
  https://github.com/nagisasaka/lucky-vibe-kanban.git \
  2e5a7fa7db4fc6e86388143ff8265066799e6d55 /temporary/path/source
git -C /temporary/path/source rev-list --count HEAD
git -C /temporary/path/source log -2 --format='%H %s'
git -C /temporary/path/source show --stat HEAD^
git -C /temporary/path/source count-objects -vH
```

The helper and Git reads exited 0; HEAD matched that SHA and full-object fsck
passed. The checkout contained 3,142 reachable commits and one 115.63 MiB object
pack. Its parent was `b5563ce25a291a2867a6eaa67e55adb05f6df183`, with the message
`Update OpenWiki to 0.9.0 through the public host protocol`. The temporary
checkout was removed after the checks. This measures an anonymous public Git
fetch and the helper, not an image layer or the final distribution size.

Final-image and retained-volume upgrade checks remain unexecuted in this
environment: Docker, Podman, BuildKit, nerdctl, buildah and OCI runtime commands
are absent, no Docker/Podman socket or configured remote endpoint is available,
and the execution bridge lists no registered runners. Installing a privileged
daemon or changing host permissions is outside this verification. The local
helper/fixture results must not be substituted for those image checks.
