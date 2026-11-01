// Browser-only fixture: no database, native files, provider or live server calls.
export function installFixtureApi() {
  const calls: string[] = [];
  Object.defineProperty(window, "__settingsFixtureCalls", { value: calls });
  const params = new URLSearchParams(location.search);
  const role = params.get("role") ?? "admin";
  const mode = params.get("auth") ?? "basic";
  const config = {
    config_version: "v2",
    theme: "SYSTEM",
    language: params.get("lang") === "en" ? "EN" : "JA",
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
    const json = (data: unknown, status = 200) =>
      Promise.resolve(
        new Response(JSON.stringify(data), {
          status,
          headers: { "Content-Type": "application/json" },
        }),
      );
    const success = (data: unknown) => json({ success: true, data });
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
            : { id: "fixture-user", admin: role === "admin" },
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
    if (path === "/api/web-push" || path === "/api/web-push/status")
      return json({ public_key: null, registrations: [], projects: [] });
    if (path === "/api/profiles")
      return success({
        content: '{"executors":{"CODEX":{"DEFAULT":{}}}}',
        path: "/fixture/profiles.json",
        revision: "fixture",
        sensitive_values_included: false,
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
