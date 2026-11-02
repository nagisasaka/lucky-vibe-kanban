import type {
  AgentToolView,
  AgentToolInventoryView,
} from "../../../shared/types";

// Every operation is in-memory. Definitions are intentionally withheld until a confirmed read.
export function createAgentToolsFixture(params: URLSearchParams) {
  const capabilities = {
    editable: true,
    removable: true,
    toggleable: true,
    exportable: true,
    installable: true,
  };
  const tool: AgentToolView = {
    provider: "codex",
    scope: "user",
    kind: "mcp_server",
    name: "MCP_RAW",
    native_path: "/fixture/raw-tool",
    state: "enabled",
    capabilities,
    revision: "revision-1",
    error: null,
    definition: {
      type: "mcp_server",
      data: {
        transport: "stdio",
        command_configured: true,
        args_count: 0,
        url_configured: false,
        env_count: 1,
        header_count: 0,
        has_provider_extensions: true,
      },
    },
  };
  const skill: AgentToolView = {
    ...tool,
    kind: "skill",
    name: "SKILL_RAW",
    native_path: "/fixture/skill",
    revision: "skill-revision-1",
    definition: { type: "skill", data: { file_count: 2, has_assets: true } },
  };
  const inventory: AgentToolInventoryView = {
    providers: [
      {
        provider: "codex",
        installed: true,
        items: [tool, skill],
        errors:
          params.get("toolsState") === "providerError"
            ? ["fixture private parse detail"]
            : [],
        limitations: [],
      },
      {
        provider: "claude_code",
        installed: true,
        items: [],
        errors: [],
        limitations: [],
      },
    ],
    errors: [],
  };
  const calls: Array<{ path: string; body: Record<string, unknown> }> = [];
  let failure = params.get("toolsFailure");
  Object.defineProperty(window, "__agentToolsFixture", {
    value: {
      calls,
      setFailure: (code: string | null) => {
        failure = code;
      },
    },
  });
  const json = (data: unknown) =>
    new Response(JSON.stringify({ success: true, data }), {
      headers: { "Content-Type": "application/json" },
    });
  const error = (code: string) =>
    new Response(
      JSON.stringify({
        success: false,
        error_data: { code, message: "fixture private error body" },
        message: "fixture private top-level message",
      }),
      { headers: { "Content-Type": "application/json" } },
    );
  return (
    url: URL,
    method: string,
    body: Record<string, unknown>,
  ): Response | Promise<Response> | undefined => {
    if (!url.pathname.startsWith("/api/agent-tools")) return;
    if (method === "GET") {
      if (params.get("toolsState") === "error")
        return error(failure ?? "invalid_configuration");
      return json(inventory);
    }
    calls.push({ path: url.pathname, body });
    if (failure) return error(failure);
    const target = (body.target ?? body.source) as {
      name: string;
      provider: string;
    };
    const item =
      inventory.providers
        .flatMap((p) => p.items)
        .find(
          (i) => i.name === target?.name && i.provider === target?.provider,
        ) ?? tool;
    if (url.pathname.endsWith("/toggle")) {
      item.state = body.enabled ? "enabled" : "disabled";
      item.revision = "revision-2";
      return json(item);
    }
    if (url.pathname.endsWith("/reveal"))
      return json({ native_path: item.native_path });
    if (url.pathname.endsWith("/read-definition")) {
      if (body.confirmed_sensitive_read !== true)
        return error("invalid_request");
      return json(
        item.kind === "skill"
          ? {
              type: "skill",
              data: {
                description: "Raw Skill description 原文",
                files: [
                  {
                    path: "SKILL.md",
                    content_base64: btoa(
                      unescape(encodeURIComponent("# Raw Skill 日本語")),
                    ),
                  },
                  { path: "asset.txt", content_base64: btoa("Raw asset") },
                ],
              },
            }
          : {
              type: "mcp_server",
              data: {
                transport: "stdio",
                command: "fixture-private-native-command",
                args: [],
                env: { TOKEN: "fixture-sensitive-value" },
                headers: {},
                source_metadata: {},
              },
            },
      );
    }
    if (url.pathname.endsWith("/copy"))
      return json({
        item: { ...item, provider: body.target_provider },
        warnings: [
          "Provider-specific fields remain at the source and were not written to the target provider.",
        ],
      });
    if (url.pathname.endsWith("/remove")) {
      inventory.providers[0].items = inventory.providers[0].items.filter(
        (i) => i !== item,
      );
      return json(null);
    }
    if (method === "POST") {
      inventory.providers[0].items.push({
        ...tool,
        ...target,
        name: target.name,
        revision: "created-revision",
      } as AgentToolView);
    }
    return json(item);
  };
}
