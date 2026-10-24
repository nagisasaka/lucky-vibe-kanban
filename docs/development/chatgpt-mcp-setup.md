---
title: Connect ChatGPT to your LVK server
description: Configure the opt-in OAuth MCP endpoint and install the LVK workflow plugin without changing existing agent authentication.
---

# Connect ChatGPT to LVK

> **Historical setup for releases before Auth0 server access.** The current implementation
> rejects `LVK_MCP_CONFIG` and uses Auth0 JWT verification with LVK membership.
> Use [the current authentication guide](../self-hosting/authentication.mdx) for
> setup, the separate preregistered ChatGPT client, and migration. The introspection,
> public-client and allowlist recipe below is retained only for old releases.
> The internal Astra assistant has also been retired; it is no longer a UI option.

This feature adds an optional authenticated MCP endpoint to the existing Linux
LVK server. It uses Streamable HTTP and an external OAuth authorisation server
with RFC 7662 introspection. It does not need an upper-model API key, migrate
Astra or change Codex authentication. Monitoring and automatic notifications
remain off. Implementation progress and scoped test evidence are recorded in
[the implementation record](chatgpt-mcp.md). ChatGPT account connection has **not
been tested in this workspace**.

## Prepare the identity provider

Use a maintained OAuth provider supporting authorisation-code flow, PKCE S256,
resource indicators, discovery and authenticated introspection. Register a public
client called `lvk-chatgpt` for ChatGPT, using token endpoint authentication
`none`. Register the exact callback shown by the ChatGPT connection setup; do not
use a wildcard. This follows the [OpenAI authentication contract](https://developers.openai.com/plugins/build/auth).

Keep the LVK resource server's confidential introspection client separate from
that public client. The resource identifier is the **exact public MCP URL**,
including any subpath and `/mcp`. Enable `lvk:read`, `lvk:write` and `lvk:merge`
scopes for this connection. Initially request read only; grant write/merge through
the host's authorisation flow when needed. The LVK server still enforces scopes,
the configured owner subject and Project allowlist on each call.

For an existing Keycloak deployment, its [MCP configuration guide](https://www.keycloak.org/securing-apps/mcp-authz-server)
requires the experimental `resource-indicators` feature for resource processing.
Set the resource server client's `resource_url` attribute to your MCP URL. Add
an Audience mapper on the pre-registered ChatGPT client targeting that resource
client, and configure the three LVK scopes. Require PKCE S256 and the exact
callback. Use the realm's HTTPS issuer and introspection endpoint in the LVK
configuration below. This is a provider setup recipe, **not a tested Keycloak
rollout or a claim of stable resource-indicator support**. Verify the provider
version's documented feature status before deployment.

Verify with a test login that introspection returns `active: true`, the exact
issuer, the owner's stable `sub`, an `aud` containing your MCP URL, a future
`exp`, and the granted scopes. LVK rejects mismatches; do not weaken validation
to accommodate a misconfigured provider. Never paste access tokens, refresh
tokens or client secrets into ChatGPT or commit them.

## Configure LVK and HTTPS

Copy [the configuration example](../../deploy/examples/chatgpt-mcp.json) to an
operator-owned configuration file. Replace every example value:

| Field | Value |
| --- | --- |
| `public_base_url` | Public LVK base, such as `https://lvk.example/team`, without a trailing slash |
| `issuer` | Exact HTTPS issuer returned by the provider |
| `introspection_endpoint` | Provider's HTTPS RFC 7662 endpoint |
| `introspection_client_id` | Confidential resource-server client ID |
| `introspection_secret_env` | Name of the server environment variable containing its secret |
| `owner_subject` | Verified owner's stable provider subject, not an email or a model-supplied ID |
| `project_ids` | Existing local Project UUIDs permitted through this connection |

Set `LVK_MCP_CONFIG` to that file's absolute server path. Supply the named secret
through your existing service/container secret injection. Do not add the secret
to the JSON or plugin package. Enabling the configuration requires your normal
reviewed server rollout; these instructions do not authorise restarting running
jobs. With `LVK_MCP_CONFIG` absent, the new endpoint is not registered.

For the supplied server image, place the configured JSON at
`deploy/server/secrets/chatgpt-mcp.json` alongside the existing TLS/Basic files.
The optional `deploy/server/compose.mcp.yaml` adds `LVK_MCP_CONFIG` and requires
`LVK_MCP_INTROSPECTION_SECRET` from your secure Compose environment. Include that
file with the existing Compose files during your authorised rollout. Keep
`introspection_secret_env` set to `LVK_MCP_INTROSPECTION_SECRET` when using this
override. Do not print rendered Compose configuration containing the secret.
This override applies only to the `lvk` service, not the optional staging or
source-development instances.

The supplied launcher reads the same `LVK_MCP_CONFIG` file and adds two exact
nginx locations on `LVK_HOST`. Only `/mcp` receives Bearer credentials. Both
locations remove cookies, Basic/proxy credentials and relay-signature headers;
all UI/API and preview routes retain their existing authentication. Without the
configuration, no Basic exception is generated. Source-development uses its
Rust backend directly for these locations, bypassing Vite. The launcher accepts
only a canonical HTTPS URL on `LVK_HOST` with plain alphanumeric, underscore or
hyphen path segments. A public path prefix maps these two endpoints to the
backend's root paths. The launcher also serves the existing application beneath
that prefix, injecting an `lvk-basepath` meta tag into HTML for the local router.
Basic authentication still protects those documents, and preview HTML remains
blocked under both paths. Root API and asset routes remain required on the same
LVK origin; this is an alias for one LVK instance, not prefix isolation for hosting
unrelated applications. Reserved `api`, `assets`, `src` and `mcp` prefixes are
rejected. The unprefixed application remains available.

For a separately managed reverse proxy, route public `<base>/mcp` and
`<base>/.well-known/oauth-protected-resource/mcp` to the corresponding LVK paths.
If you manage a separate proxy and use a public subpath, preserve this mapping
and supply the same trusted `lvk-basepath` HTML meta tag; rewriting the endpoint
alone does not configure the browser router. Keep the existing authenticated
root API/assets available and apply the preview denial to prefixed paths too.
Keep the public Host/Origin and Bearer header intact. Exempt **only these exact
MCP paths** from a proxy's Basic-auth challenge: MCP supplies its own Bearer
validation, while resource metadata is intentionally public. Preserve the
existing UI/API authentication. Do not expose the internal backend port or exempt
all `/api` routes. Basic credentials and UI cookies are not MCP access tokens.

Before connecting, verify metadata and an unauthenticated MCP request: metadata
must identify the expected resource and issuer, and `/mcp` must reject anonymous
access with `401` and a Bearer resource-metadata challenge. Then use an
authenticated MCP client to initialise, list tools and call `get_connection`.
These checks must use the actual public URL, not just loopback. Invalid tokens
return `401`, missing read permission `403`, and an unavailable identity provider
`503`. Write/merge scope challenges request reauthorisation without performing
the operation.

## Build your plugin package

The source package is [plugins/lvk](../../plugins/lvk). It follows the
[portable Agent Plugins format](https://developers.openai.com/plugins/build/plugins):
`plugin.json`, a remote `mcp.json`, and one `lvk-work` skill. The sample URL uses
`.invalid` intentionally. Create a separate configured distribution:

```bash
python3 scripts/chatgpt-mcp/configure.py \
  --public-base-url https://lvk.example/team \
  --output /tmp/my-lvk-plugin
```

Use your real public base. Enter the registered public client ID in ChatGPT's OAuth connection setup. The destination must
not already exist. The script creates `plugins/lvk` and a relative
`.agents/plugins/marketplace.json` inside that destination. It does not install,
publish, alter global client settings or include credentials. The packaged skill
and MCP dependency use the same URL. Do not upload server configuration or secrets
with the package.

The published Agent Plugins 1.0 MCP schema currently permits `type`, `url` and
`headers` for HTTP servers, but rejects the `extensions` field shown in OpenAI's
OAuth package example (checked 2026-10-13). This package uses the portable subset
and configures the pre-registered OAuth client in the host's connection setup.
It contains no auth bypass or static Bearer header. LVK's discovery challenge
and token checks remain mandatory. Revalidate the schemas before adopting a
future package-level OAuth extension.

## Connect and install in ChatGPT Web

1. Open [ChatGPT Plugins](https://chatgpt.com/plugins), choose **Add custom MCP
   server**, and enter your public URL ending in `/mcp`. Select OAuth and your
   pre-registered client. Complete the provider login and review the discovered
   tools. Create/install the connection and test `get_connection` in a new chat.
2. In Chat or Work on the web, select **Plugin Creator**. Attach the configured
   `SKILL.md`, its two `references/` files and `agents/openai.yaml`. Ask it to
   create a private LVK workflow plugin using those instructions and your existing
   LVK MCP app connection. Name the connection explicitly; do not create another
   anonymous connection or invent a technical app ID.
3. Review the resulting plugin's Skills and Apps sections. Confirm that the
   instructions, reference files and LVK connection are included. Complete any
   installation prompt, start a new conversation, select the LVK plugin with `@`,
   and run the checks below.

The [custom MCP connection flow](https://developers.openai.com/plugins/deploy/connect-chatgpt)
and [Web Plugin Creator workflow](https://learn.chatgpt.com/docs/build-plugins)
are separate steps. Web creation needs the relevant workspace permission and
Plugin Creator availability. If either is unavailable, record that limitation;
a local folder or a Codex CLI installation is **not** proof of Web installation.

For reproducible file-based workspace distribution, an authorised admin can
place the configured distribution in a GitHub repository and use **Admin →
Plugins → Add → Import marketplace**, selecting its directory and a pinned
commit. Review imported skills, connection and installation policy before use.
This is private workspace distribution, not a public store submission. See
[workspace import and sync](https://learn.chatgpt.com/docs/enterprise/plugin-management).
Neither GitHub publication nor workspace import has been performed by this task.

## Verify behaviour in your account

Use a disposable LVK Project and repository. Keep a record of date, account plan,
Chat/Work mode, installation route, consent behaviour and results. Do not save
personal memory contents or credentials in fixtures.

| Request/check | Expected evidence |
| --- | --- |
| Show this Project's work and questions | IDs, bounded summaries and working LVK links |
| Create this Issue; do not implement | Issue exists; no Workspace/run created |
| Implement that Issue | Acceptance receipt plus Workspace/Session/run IDs; completion is separate |
| Close the chat, then ask for progress in a new chat | Existing LVK execution continues and is found by Project/Issue |
| Give a direct LVK/Codex instruction, then ask what changed | Latest command/state appears without relying on chat history |
| Read and answer one specification question | Exact question revision; direct or repeated answers are not duplicated |
| Encounter a shell approval | Follow the LVK link; specification-answer tools cannot approve it |
| Inspect results and request test Integration | Validation, publication and Wiki outcomes remain separate |
| Open an Issue/Workspace link after login | Return to the intended object, including any public subpath |
| Disconnect/revoke the new OAuth grant | Subsequent MCP requests fail; existing jobs and Codex credentials remain intact |

Also check personal memory settings in the actual conversation mode and, if you
wish, a harmless preference of your choosing. LVK never fetches or synchronises
ChatGPT personal memory, and this integration does not promise recall on every
request. Record Chat and Work separately when testing both.

## Archive a Workspace through remote MCP

The remote catalog exposes `prepare_workspace_archive` and
`confirm_workspace_archive`; stdio's `update_workspace` is a separate tool.
Supply explicit `project_id` and `workspace_id` at both stages. Both require
`lvk:read` and `lvk:write`, permitted Project access and matching Workspace
membership. Preparation does not delete files or set `archived`.

Show the returned target and exact message 「ワークツリーが削除されます」 and
ask once. If confirmed, submit its `confirmation_token` with a stable
`request_id`; cancellation means not submitting. Tokens expire after ten minutes,
are single-use, and are bound to the authenticated issuer/subject and exact
Project/Workspace/target. Do not transfer UI/stdio tokens to remote or vice versa.
Busy checks run before confirmation and again before physical deletion.

After response loss, use `get_operation(project_id, request_id)` or repeat the
exact confirm request with the same request ID. Inspect `result.archived`,
`result.worktree_deleted` and `result.deletion_error` independently. Physical
failure still archives and reports the actual deletion error; do not retry
silently. `get_workspace` exposes the current two state flags. Uncertain results
require investigation. Server restarts invalidate pending confirmation tokens,
while durable operation receipts remain available.

## Refresh, disconnect and recover

After a reviewed server update changes tool metadata, open the custom MCP
connection and choose **Refresh**, then test in a new chat. Updating MCP metadata
does not replace packaged Skill files: edit/reinstall the private plugin or sync
the configured workspace source separately.

After a lost write response, call `get_operation` with its original `request_id`.
Reuse the same key only for identical input; an uncertain outcome requires
inspection before another mutation. Do not equate acceptance with finished work.

To withdraw access, disconnect/uninstall the LVK plugin and revoke its OAuth
grant at the provider. Removing `LVK_MCP_CONFIG` in your normal rollout disables
the endpoint. Retire the new introspection credential only when no configured
consumer still needs it. Preserve the database receipts for reconciliation;
withdrawal does not delete work or intentionally cancel existing execution.
No withdrawal step re-enables Astra monitoring or changes Codex authentication.

Code, requirements, questions and selected evidence returned by these tools go
to the conversation host and follow your ChatGPT data settings. Limit the Project
allowlist to what you intend to expose. No personal conversation synchronisation,
automatic notifications, MCP Events or conversation UI is installed.

## Reproduce package validation

Run in the feature checkout. These commands create only local validation outputs;
use an isolated Python environment. The Python packages are validation tools,
not LVK runtime dependencies.

```bash
python3 -m venv /tmp/lvk-package-check
/tmp/lvk-package-check/bin/pip install jsonschema==4.23.0 PyYAML==6.0.2
mkdir -p /tmp/lvk-package-schemas
curl -fsSL https://agent-plugins.org/schemas/1.0.0/plugin.schema.json \
  -o /tmp/lvk-package-schemas/plugin.schema.json
curl -fsSL https://agent-plugins.org/schemas/1.0.0/mcp.schema.json \
  -o /tmp/lvk-package-schemas/mcp.schema.json
LVK_MCP_TOOL_CATALOG=/tmp/lvk-tools.json \
  cargo test -p server --lib remote_mcp::tests::plugin_tool_catalog -- --nocapture
/tmp/lvk-package-check/bin/python scripts/chatgpt-mcp/validate.py \
  --plugin plugins/lvk --catalog /tmp/lvk-tools.json --schemas /tmp/lvk-package-schemas
/tmp/lvk-package-check/bin/python -m unittest discover \
  -s scripts/chatgpt-mcp -p test_package.py -v
```

Repeat the validator with `--plugin` pointing to your configured output. It checks
both manifests against the published schemas, bundled references, dependency URL
consistency, Skill tool names, sample inputs and scopes against the actual Rust
catalog. The sample IDs are fixtures, not calls to a running LVK. This static
validation does not prove model selection, installation, OAuth login or browser
behaviour; the account checklist above remains necessary.
