import { createAgentToolsFixture } from "./agent-tools";
import { createDesignNoteFixture } from "./design-notes";
// Browser-only fixture: no database, native files, provider or live server calls.
export function installFixtureApi() {
  const calls: string[] = [];
  const localeSaves: Array<{
    language: string;
    browserLanguage: string | null;
  }> = [];
  Object.defineProperty(window, "__fixtureLocaleSaves", { value: localeSaves });
  Object.defineProperty(window, "__settingsFixtureCalls", { value: calls });
  const params = new URLSearchParams(location.search);
  const designNotes = createDesignNoteFixture(params);
  const agentTools = createAgentToolsFixture(params);
  const role = params.get("role") ?? "admin";
  const mode = params.get("auth") ?? "basic";
  const config = {
    config_version: "v2",
    theme: "SYSTEM",
    language:
      params.get("browserLanguage") === "true"
        ? "BROWSER"
        : params.get("lang") === "en"
          ? "EN"
          : "JA",
    executor_profile: { executor: "CODEX", variant: null },
    disclaimer_acknowledged: true,
    onboarding_acknowledged: true,
    notifications: {
      sound_enabled: false,
      push_enabled: false,
      sound_file: "ABSTRACT_SOUND1",
    },
    editor: { editor_type: "VS_CODE", entries: [] },
    github: {},
    analytics_enabled: false,
    workspace_dir: null,
    git_branch_prefix: "fixture",
    hidden_agents: [],
    pr_auto_description_enabled: true,
    pr_auto_description_prompt: null,
    commit_reminder_enabled: true,
    commit_reminder_prompt: null,
    showcases: {},
    send_message_shortcut: "ModifierEnter",
    integration_skip_validation: false,
  };
  let runtime = {
    revision: 0,
    source: "default",
    settings: {
      files_enabled: true,
      execution_enabled: false,
      file_roots: ["/fixture"],
    },
  };
  const archiveRestoreCalls: unknown[] = [];
  Object.defineProperty(window, "__archiveRestoreCalls", {
    value: archiveRestoreCalls,
  });
  const prCalls: unknown[] = [];
  Object.defineProperty(window, "__prFixtureCalls", { value: prCalls });
  const approvalCalls: unknown[] = [];
  Object.defineProperty(window, "__approvalCalls", { value: approvalCalls });
  const nativeFetch = window.fetch.bind(window);
  window.fetch = async (input, init) => {
    const url = new URL(
      input instanceof Request ? input.url : String(input),
      location.href,
    );
    if (!url.pathname.startsWith("/api/")) return nativeFetch(input, init);
    const path = url.pathname;
    calls.push(path);
    const method = init?.method ?? "GET";
    const body = init?.body ? JSON.parse(String(init.body)) : {};
    const toolsResponse = agentTools(url, method, body);
    if (toolsResponse) return toolsResponse;
    const noteResponse = designNotes(url, method, body);
    if (noteResponse) return noteResponse;
    const json = (data: unknown, status = 200) =>
      Promise.resolve(
        new Response(JSON.stringify(data), {
          status,
          headers: { "Content-Type": "application/json" },
        }),
      );
    const success = (data: unknown) => json({ success: true, data });
    if (params.has("repo-load-error") && path === "/api/repos")
      return json(
        { success: false, message: "Private repository fixture secret" },
        503,
      );
    if (params.has("pr") && path === "/api/repos/raw-repo/branches")
      return success([
        {
          name: "feature/raw-target-原文",
          is_current: false,
          is_remote: false,
        },
      ]);
    if (path === "/api/workspaces/raw-pr-workspace/messages/first")
      return success("Raw initial title 原文\n\nRaw initial body 原文");
    if (
      path === "/api/workspaces/raw-pr-workspace/pull-requests" &&
      method === "POST"
    ) {
      prCalls.push({ path, body });
      if (params.get("pr") === "throw")
        throw new TypeError("Raw PR secret /internal/path");
      return json({
        success: false,
        message: "Raw PR secret /internal/path",
        error_data: { type: "cli_not_logged_in", provider: "git_hub" },
      });
    }

    if (path === "/api/approvals/raw-approval/respond" && method === "POST") {
      approvalCalls.push(body);
      if (params.has("approval-error"))
        return json(
          { success: false, message: "Raw approval provider secret" },
          500,
        );
      if (params.has("approval-hold"))
        await new Promise<void>((resolve) =>
          Object.defineProperty(window, "__finishApproval", {
            value: resolve,
            configurable: true,
          }),
        );
      return success(body.status);
    }

    if (path === "/api/board/issues/raw-issue/unarchive" && method === "POST") {
      archiveRestoreCalls.push(body);
      if (params.has("restore-lost-response"))
        throw new Error("Raw restore transport secret");
      return success({
        request_id: body.request_id,
        status: params.has("restore-uncertain") ? "uncertain" : "succeeded",
        result: params.has("restore-uncertain")
          ? JSON.stringify({ message: "Raw receipt secret" })
          : "{}",
      });
    }

    if (
      path === "/api/workspaces/raw-wiki-workspace/wiki" &&
      method === "GET"
    ) {
      if (params.get("wiki-mode") === "error")
        return json(
          { success: false, message: "Raw internal secret / private-path" },
          503,
        );
      const repoId = url.searchParams.get("repo_id");
      return success({
        workspace_id: "raw-wiki-workspace",
        repo_id: repoId,
        repo_name: "Raw repository",
        repo_display_name: "Raw repository 原文",
        source: "current_workspace",
        source_links: [
          { source: "LVK-7", project_id: "raw-project", issue_id: "raw-issue" },
        ],
        wiki: {
          exists: true,
          config: null,
          index: {
            path: "index.md",
            metadata: null,
            content: "# Raw home 原文\n[Raw link](concepts/raw.md)",
          },
          pages: [
            {
              path: "concepts/raw.md",
              content: "# Raw article 原文\nRaw model documentation {{value}}",
              metadata: {
                schema_version: 1,
                title: "Raw title 原文",
                summary: "Raw summary 原文",
                language: "ja",
                tags: ["RawTag 原文"],
                sources: ["LVK-7"],
                repos: ["raw-repo-one"],
                created: "2026-01-01",
                updated: "2026-01-02",
              },
            },
          ],
        },
      });
    }
    if (path === "/api/workspaces/raw-file-workspace" && method === "GET")
      return success({
        id: "raw-file-workspace",
        usage: params.get("file-usage") ?? "interactive",
      });
    if (
      path.startsWith("/api/workspaces/raw-file-workspace/files/") &&
      method === "GET"
    ) {
      const mode = params.get("files-mode");
      const entry = (name: string, kind = "file", value = name) => ({
        workspace_id: "raw-file-workspace",
        repo_id: "raw-repo",
        repo_name: "Raw repo 原文",
        name,
        path: value,
        kind,
        size_bytes: 1536,
        modified_at: null,
        mime_type: kind === "directory" ? null : "text/plain",
        is_binary: false,
      });
      if (path.endsWith("/tree")) {
        return success({
          repos: [
            {
              workspace_id: "raw-file-workspace",
              repo_id: "raw-repo",
              repo_name: "Raw repo 原文",
              repo_display_name: "Raw repo 原文",
              truncated: true,
              entries: [entry("src", "directory"), entry("Raw 原文.txt")],
            },
          ],
        });
      }
      if (path.endsWith("/directory")) {
        return success({
          workspace_id: "raw-file-workspace",
          repo_id: "raw-repo",
          repo_name: "Raw repo 原文",
          path: "src",
          entries: [entry("child 原文.txt", "file", "src/child 原文.txt")],
          truncated: true,
        });
      }
      if (path.endsWith("/content")) {
        if (mode === "preview-error")
          return json(
            { success: false, message: "Raw internal preview secret" },
            503,
          );
        const value = url.searchParams.get("path") ?? "Raw 原文.txt";
        return success({
          workspace_id: "raw-file-workspace",
          repo_id: "raw-repo",
          repo_name: "Raw repo 原文",
          path: value,
          name: value.split("/").at(-1),
          kind: "text",
          mime_type: "text/plain",
          language: null,
          content: "Raw file {{user}} 原文\nline 2",
          raw_url:
            "/api/workspaces/raw-file-workspace/files/raw?repo_id=raw-repo&path=" +
            encodeURIComponent(value),
          size_bytes: 1536,
          truncated: true,
        });
      }
      return json(
        { success: false, message: "Raw route not executed by fixture" },
        403,
      );
    }
    if (path === "/api/access/status")
      return json({
        mode,
        deployment: mode === "local" ? "local" : "public",
        authenticated:
          mode !== "locked" &&
          !sessionStorage.getItem("settings-fixture-logged-out"),
        management: role === "admin" && mode !== "locked" && mode !== "local",
        limited: false,
        csrf: "fixture-csrf",
        public_url: location.origin,
        forced_mode: null,
        configuration_error: false,
        user:
          mode === "locked"
            ? null
            : {
                id: "fixture-user",
                admin: role === "admin",
                display_name:
                  params.get("identity") === "missing"
                    ? null
                    : params.get("identity") === "blank"
                      ? "   "
                      : mode === "basic"
                        ? "sample-basic-user"
                        : "Sample Auth0 Person",
              },
      });
    if (path === "/api/access/logout" && params.get("allowLogout") === "true") {
      if (params.get("logoutFailure") === "true")
        return json({ error: "Fixture logout failed" }, 503);
      sessionStorage.setItem("settings-fixture-logged-out", "true");
      return json({});
    }
    if (path.startsWith("/api/access/")) {
      if (role !== "admin")
        return json({ error: "Fixture: administrator required" }, 403);
      if (path.endsWith("/basic") && params.get("allowHandoff") === "true")
        return json({});
      if (path.endsWith("/auth0/chatgpt-credentials")) {
        if (body.action === "status")
          return json({ available: true, client_id: "fixture-chat" });
        if (body.action === "save")
          return json({ available: true, client_id: body.client_id });
      }
      if (path.endsWith("/configuration"))
        return json({
          public_url: location.origin,
          callback_url: location.origin + "/api/access/callback",
          logout_url: location.origin,
          resource: "fixture",
          scopes: [],
          current: {
            mode,
            username: "demo",
            issuer: "https://fixture.invalid/",
            client_id: "fixture-client",
            secret_configured: true,
            mcp_enabled: false,
          },
        });
      if (path.endsWith("/confirmation")) return json({ ready: false });
      if (path.endsWith("/members"))
        return json({
          members: [
            {
              id: "fixture-user",
              display_name: "Demo administrator",
              issuer: "fixture",
              subject: "demo",
              role: "admin",
              state: "active",
            },
          ],
        });
      if (path.endsWith("/audit"))
        return json({
          events: [
            {
              id: 1,
              at: 0,
              actor: "fixture",
              action: "login",
              outcome: "sample",
            },
          ],
        });
      if (path.endsWith("/chatgpt/credentials"))
        return json({ credentials: [] });
      return json(
        { error: "Fixture: authentication mutations are disabled" },
        403,
      );
    }
    if (path === "/api/releases") {
      if (params.get("releasesState") === "error")
        return json({ message: "fixture private release detail" }, 503);
      if (params.get("releasesState") === "loading")
        return new Promise<Response>(() => {});
      return success({
        releases: [
          {
            tag_name: "v1.2.3",
            body: "Raw **release content** 原文",
            published_at: "2026-01-02T15:04:05Z",
          },
        ],
      });
    }
    if (path === "/api/info")
      return success({
        config,
        version: "settings-prototype",
        environment: {
          os_type: "linux",
          os_version: "fixture",
          os_architecture: "x86_64",
          bitness: "64",
        },
        executors: { CODEX: { DEFAULT: {} } },
        capabilities: {},
        machine_id: "fixture",
        login_status: { status: "loggedout" },
      });
    if (path === "/api/config" && method === "PUT") {
      localeSaves.push({
        language: body.language,
        browserLanguage: new Headers(init?.headers).get(
          "X-LVK-Browser-Language",
        ),
      });
      if (params.get("failSave") === "true")
        return json({ success: false, message: "Fixture save rejected" }, 409);
      Object.assign(config, body);
      return success(config);
    }
    if (path === "/api/runtime-access-settings") {
      if (method === "PUT")
        runtime = { ...body, revision: runtime.revision + 1, source: "saved" };
      return json(runtime);
    }
    if (params.has("web-push-load-error") && path === "/api/web-push")
      return json({ message: "Private push fixture endpoint secret" }, 503);
    if (path === "/api/web-push" || path === "/api/web-push/status")
      return json({ public_key: null, registrations: [], projects: [] });
    if (params.has("profile-load-error") && path === "/api/profiles")
      return json(
        { success: false, message: "Private launch profile fixture secret" },
        403,
      );
    if (path === "/api/profiles")
      return success({
        content: '{"executors":{"CODEX":{"DEFAULT":{}}}}',
        path: "/fixture/profiles.json",
        revision: "fixture",
        sensitive_values_included: false,
      });
    if (
      path === "/api/agent-settings/diff" &&
      params.get("view") === "agent-configuration"
    )
      return success({
        provider: "codex",
        files: [
          {
            file_id: "native-1",
            path: "/fixture/config.toml",
            before: "Observed native revision (values withheld)",
            after:
              "Requested settings will be applied; unrelated native values are preserved.",
            changed: true,
          },
        ],
        warnings: [
          "Persistent changes affect new provider sessions; active sessions are not restarted.",
        ],
      });
    if (path === "/api/agent-settings" && params.has("configurationError"))
      return json({
        success: false,
        error_data: {
          code: params.get("configurationError"),
          message: "PRIVATE_BODY",
          recovery: "PRIVATE_RECOVERY",
        },
      });
    if (
      path === "/api/agent-settings" &&
      params.get("view") === "agent-configuration"
    )
      return success({
        providers: [
          {
            provider: "codex",
            installed: true,
            provider_version: "fixture",
            executable_path: "/fixture/codex",
            schema_revision: "schema-1",
            capabilities: {
              readable: true,
              native_writable: true,
              profile_storage: true,
              per_run_overrides: true,
              raw_editable: true,
            },
            descriptors: [
              {
                key: { namespace: "raw", name: "number" },
                section: "general",
                label: "RAW_NUMBER",
                description: "",
                value_type: "number",
                control: "number",
                options: [],
                validation: { minimum: 2, maximum: 4 },
                supported_scopes: ["user"],
                capabilities: {
                  writable: true,
                  readable: true,
                  resettable: true,
                  profile_storable: true,
                  run_override: true,
                },
                native_locations: [],
                activation: "next_session",
              },
              {
                key: { namespace: "common", name: "reasoning" },
                section: "general",
                label: "Reasoning effort",
                description: "Default model reasoning effort.",
                value_type: "string",
                control: "select",
                options: [
                  { value: "high", label: "High" },
                  { value: "future_option", label: "Raw future option 原文" },
                ],
                validation: {},
                supported_scopes: ["user"],
                capabilities: {
                  writable: true,
                  readable: true,
                  resettable: true,
                  profile_storable: true,
                  run_override: true,
                },
                native_locations: [],
                activation: "next_session",
              },
            ],
            native_files: [
              {
                id: "native-1",
                path: "/fixture/config.toml",
                format: "toml",
                scope: "user",
                exists: true,
                parse_status: "parsed",
                revision: "revision-1",
                writable: true,
                raw_editable: true,
              },
            ],
            effective_settings: [
              {
                key: { namespace: "raw", name: "number" },
                sources: [],
                effective_value: 2,
                effective_source: "user",
                warnings: [],
              },
            ],
            unknown_native_nodes: [],
            withheld_setting_keys: [],
            limitations: [],
            errors: [],
          },
        ],
        errors: [],
      });
    if (path === "/api/agent-settings")
      return success({ providers: [], errors: [] });
    if (path === "/api/resource-coordination/snapshot")
      return success({ resources: [], holders: [], queue: [] });
    if (
      path === "/api/repos" ||
      path === "/api/tags" ||
      path === "/api/agent-settings/profiles"
    )
      return success([]);
    // Fail closed for unsupported fixture endpoints instead of proxying live APIs.
    return json(
      {
        success: false,
        message: "Not available in the isolated settings fixture",
      },
      503,
    );
  };
}
