// Public MCP stdio client for model-free OpenWiki compatibility fixtures.
import { spawn } from "node:child_process";
import { createInterface } from "node:readline";

export async function openWikiMcp(binary, root, env) {
  const child = spawn(binary, ["mcp", "--host", "codex"], {
    cwd: root,
    env,
    stdio: ["pipe", "pipe", "inherit"],
  });
  const pending = new Map();
  let sequence = 0;
  const lines = createInterface({ input: child.stdout });
  lines.on("line", (line) => {
    let frame;
    try {
      frame = JSON.parse(line);
    } catch {
      return;
    }
    const waiter = pending.get(frame.id);
    if (!waiter) return;
    pending.delete(frame.id);
    if (frame.error) waiter.reject(new Error(JSON.stringify(frame.error)));
    else waiter.resolve(frame.result);
  });
  const fail = (error) => {
    for (const waiter of pending.values()) waiter.reject(error);
    pending.clear();
  };
  child.on("error", fail);
  child.on("exit", (code) => fail(new Error(`MCP exited: ${code}`)));
  function call(method, params) {
    const id = ++sequence;
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        pending.delete(id);
        reject(new Error(`Timeout: ${method}`));
      }, 30000);
      pending.set(id, {
        resolve: (value) => {
          clearTimeout(timeout);
          resolve(value);
        },
        reject: (error) => {
          clearTimeout(timeout);
          reject(error);
        },
      });
      child.stdin.write(
        `${JSON.stringify({ jsonrpc: "2.0", id, method, params })}\n`,
      );
    });
  }
  async function close() {
    lines.close();
    if (child.exitCode === null && child.signalCode === null) {
      const exited = new Promise((resolve) => child.once("exit", resolve));
      const force = setTimeout(() => child.kill("SIGKILL"), 5000);
      child.kill("SIGTERM");
      await exited;
      clearTimeout(force);
    }
  }
  try {
    await call("initialize", {
      protocolVersion: "2024-11-05",
      capabilities: {},
      clientInfo: { name: "lvk-openwiki-smoke", version: "1" },
    });
    child.stdin.write(
      `${JSON.stringify({ jsonrpc: "2.0", method: "notifications/initialized" })}\n`,
    );
  } catch (error) {
    await close();
    throw error;
  }
  return {
    call,
    close,
    tool: async (name, args) =>
      mcpData(await call("tools/call", { name, arguments: args })),
  };
}

export function mcpData(result) {
  if (result.isError) throw new Error(JSON.stringify(result));
  return (
    result.structuredContent ??
    JSON.parse(result.content.find((b) => b.type === "text").text)
  );
}
