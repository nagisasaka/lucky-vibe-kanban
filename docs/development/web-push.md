# Browser Web Push and ChatGPT consultation

Web Push is optional and off until each authenticated user explicitly registers a
browser for a Project in **Settings → General → Web Push**. The existing native
sound/desktop notification settings are separate. Registration requires a secure
context, Push API, a Service Worker and notification permission. Denying permission
does not disable any LVK functionality.

## Server configuration

Configure the following on the LVK server process/container, then restart it through
your normal deployment procedure. This feature does not deploy or rotate keys for you.

- `LVK_WEB_PUSH_PRIVATE_KEY_FILE`: path to a persistent P-256 EC private key in PEM
  format, readable only by the server account. Mount it as a secret; never put it
  in the repository, frontend build, URL or logs.
- `LVK_WEB_PUSH_SUBJECT`: an administrator contact such as `mailto:admin@example.com`.

For example, create a new dedicated key in your protected server configuration directory:

```sh
umask 077
openssl ecparam -name prime256v1 -genkey -noout -out web-push-private.pem
```

The public application-server key is derived from this PEM and returned to logged-in
browsers. Missing/invalid configuration leaves registration unavailable and the rest
of LVK operational. Preserve the key across restarts. After deliberate key rotation,
use **ON / 再登録** on each browser; the UI replaces a subscription with a different key.

Production use needs HTTPS and outbound HTTPS to browser push services. Supported
endpoint families are Google FCM (`/wp/` and legacy `/fcm/send/`), Mozilla autopush, Apple Web Push and Microsoft
`*.notify.windows.com`. Arbitrary hosts, HTTP, credentials, non-default ports and
redirects are rejected. A new vendor requires an explicit allowlist/code review.
Never configure an arbitrary user-supplied relay endpoint. The Google paths follow
[Chromium’s endpoint constants](https://chromium.googlesource.com/chromium/src/+/refs/tags/141.0.7390.94/components/push_messaging/push_messaging_constants.cc).

## Privacy and authorisation

Registrations belong to the authenticated LVK user, Project and browser endpoint.
The current local Board is shared among active server members; it does **not** have
private per-Project ACLs. Local mode has one local identity, Basic has its configured
owner, and OAuth uses the active member identity. Push does not add broader access.
Internal-agent and MCP identities cannot register browsers. A browser endpoint cannot
silently move between owners; switching accounts creates a fresh browser subscription.

Endpoint and encryption keys are sealed using the existing server-access encryption
key. The DB contains only an endpoint hash outside this envelope. Keep database and
server-access key backups protected together. Delivery revalidates the member and authentication-configuration revision. Member
disable and access configuration changes stop it. Device consent intentionally
outlives browser idle/absolute expiry and logout so overnight jobs can notify. OFF
revokes consent. The envelope grants no API access and does not renew login sessions;
opening details still requires login. Re-register after the 90-day consent expiry.

Lock-screen text contains only LVK, event category and generic instructions. No Issue
title, prompt, command, response, exception text or secret is included. IDs/relative
navigation URLs are inside the encrypted push payload. Detailed data and all answers
or approvals remain behind LVK authentication. ChatGPT consultation copies only IDs
and a reference URL; it never copies question text, code, tokens or log contents.

**OFF** deletes that registration and queued deliveries. You can remove other listed
registrations for your own account. Removing the last registration on the current
browser also unsubscribes it locally. A bounded in-flight send settles before OFF
returns; an already accepted OS/push-service notification cannot be recalled.
404/410 responses remove expired registrations. Registrations expire after 90 days
without re-registration by the background dispatcher.

## Lifecycle and delivery

DB triggers enqueue only committed canonical Codex `input_requested`,
`approval_requested` and AgentRun transitions to `succeeded`, `failed`, `cancelled`,
`crashed` or `audit_failed`. Nonblocking `async_questions`, assistant messages,
provider turn completion, script/test completion and Integration/publication events
are not triggers. Run success says nothing about tests, merge or publication.
A Workspace must currently be linked to the subscribed Project. Only events after
registration are queued; existing history is not replayed as a notification burst.

The outbox is in the same DB transaction as event admission/projection. A control is
deduplicated per attempt/type/reference; a terminal notification per Run/attempt and
browser registration. The worker rechecks unresolved questions and originating
attempts, discarding answered/expired questions and obsolete retry results. It does
not infer crashes from a disconnected browser or agent transport.

Delivery is best effort: up to five attempts for network/429/5xx failures, exponential
backoff (30–480 seconds), a 120-second crash-recovery reservation, a one-hour job
lifetime and five-minute push TTL. Successful sends and permanent 3xx/4xx failures finish
the job; receipts are retained for seven days. Endpoint values and network error
strings are not logged. Multiple subscribed browsers receive independent jobs.
A stable push Topic, notification tag and bounded persistent Service Worker receipts
suppress retries. Network acceptance can be ambiguous: exactly-once delivery is not
guaranteed, and clearing browser storage clears client-side receipts.

The root-scoped `/lvk-push-sw.js` has **no fetch handler** and caches no authenticated
pages or API responses. Clicks focus an exact matching page or open the authenticated
Project/Issue/Workspace route with `session_id` and `agent_run_id`. The auth gate keeps
the requested URL through login. A result reference in the composer identifies the
notified Run even when a newer Run exists; the timeline remains the full result source.

## ChatGPT consultation

On a blocking question/approval or a completed Run, use **ChatGPTに相談 — 依頼文をコピー**,
then **ChatGPTを開く**, and paste. If Clipboard API access fails, the selectable text
area is the fallback. No undocumented ChatGPT deep link or automatic submission is
used. Connect the existing LVK MCP to ChatGPT to retrieve the reference. `get_question`
and `answer_question` remain subject to their existing scope/revision rules; execution
approvals must be decided in LVK. This feature never answers, approves or merges.

## Manual acceptance checks

Use an isolated authorised HTTPS deployment with a configured key and ordinary
Codex runs. Do not mutate a production DB to manufacture events. Desktop Chrome on
Windows/macOS and Android Chrome are primary checks. Firefox and Safari should also
be checked when available. iOS/iPadOS requires a home-screen Web app and a supporting
OS (WebKit introduced support in 16.4); ordinary Safari tabs are not equivalent.
See [WebKit's support conditions](https://webkit.org/blog/13878/web-push-for-web-apps-on-ios-and-ipados/).

1. Before permission, verify the app works. Click ON, deny permission, verify the
   explanation and normal questions still work; reset permission and allow ON.
2. Subscribe two browsers to Project A, another to B. Close LVK tabs. Trigger a
   blocking specification question in A: only A's devices receive generic text.
   Reopen a notification and verify Project, Issue, Workspace and Session identities.
3. Trigger execution approval. Review/approve/deny using the existing LVK controls;
   the ChatGPT copy action must not submit an answer or approval. Repeat with a
   resolved question before the next worker tick and confirm no stale notification.
4. Observe successful, failed, cancelled and abnormal Run endings. Confirm that
   messages do not claim tests passed, Integration succeeded or main was published.
   Retry a Run and check old results/questions are not attributed to the new attempt.
5. Click while the matching tab exists, while all tabs are closed, and after login
   expires. Check that login returns to the requested reference. Compare copied
   Project/Issue/Workspace/Session/Run/question IDs against the existing MCP reads.
6. Check OFF on the current device and remote removal of your other registration;
   later events must not send. Verify another account cannot list/delete your entries.
   After logout, generic push should still arrive but opening it requires login.
   Disable a member before delivery and confirm no push is sent.
7. In an isolated environment, interrupt outbound connectivity/restart the server;
   restore it within an hour and verify bounded retry/dedup. Test expired browser
   endpoints and VAPID rotation/re-registration. Never log real endpoint capabilities.
8. Test mobile widths, light/dark themes, disabled notifications, non-HTTPS/unsupported
   browsers, clipboard failure and popup restrictions. Check iOS home-screen launch.

OS focus modes, battery/network policy, forced browser shutdown and revoked permission
can delay or prevent delivery. A fully stopped browser is not universally woken by
Web Push. Keep LVK's own state as the source of truth, not the presence/absence of a push.

Automated checks: `cargo test -p server web_push --lib`,
`cargo test -p server-access push_consent`,
`node --test tests/web-push/*.test.mjs`, `pnpm --dir packages/web-core exec vitest run src/shared/components/web-push`.
These do not prove live OS delivery or real ChatGPT-account interoperability.

## Parallel Issue handoff

This change does not require the parallel Codex intermediate-response MCP work.
Shared touchpoints are `crates/server/src/routes/mod.rs`, `crates/server/Cargo.toml`,
`Cargo.lock`, server-access consent helpers/tests, `ServerAccessGate.tsx`,
`packages/public/site.webmanifest`, `SessionChatBoxContainer.tsx`, General Settings
and Project search validation. The new migration is `20261018180000_web_push.sql`.
Consultation and browser settings components live under shared/components/web-push
so existing feature and shared-layer import boundaries are preserved. The existing
question service is read-only from the push worker and remains the canonical resolver; no MCP schema or approval control path is changed.
Use LVK's formal Integration workflow to reconcile changes; this Workspace does not
integrate or deploy them.
