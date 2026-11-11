// Read-only version probe through the public MCP handshake. No generation run.
// Also embedded by the Rust adapter; keep this independent of local imports.
import { spawn } from "node:child_process";
import { createInterface } from "node:readline";

const [binary, expected] = process.argv.slice(2);
const child = spawn(binary, ["mcp", "--host", "codex"], {
  stdio: ["pipe", "pipe", "ignore"],
  env: { ...process.env, OPENWIKI_TELEMETRY_DISABLED: "1" },
});
const lines = createInterface({ input: child.stdout });
let settled = false;
const finish = (error, version) => {
  if (settled) return;
  settled = true;
  clearTimeout(timer);
  lines.close();
  child.kill("SIGKILL");
  if (error) {
    console.error(error);
    process.exitCode = 1;
  } else console.log(version);
};
const timer = setTimeout(
  () => finish("OpenWiki MCP version probe timed out"),
  20000,
);
child.on("error", () => finish("Cannot start OpenWiki MCP version probe"));
child.on("exit", () =>
  finish("OpenWiki MCP exited before its version response"),
);
child.stdin.on("error", () =>
  finish("OpenWiki MCP version probe input closed"),
);
lines.on("line", (line) => {
  let frame;
  try {
    frame = JSON.parse(line);
  } catch {
    return;
  }
  if (frame.id !== 1) return;
  const info = frame.result?.serverInfo;
  if (info?.name !== "openwiki" || info.version !== expected)
    finish("OpenWiki MCP identity/version does not match the LVK pin");
  else finish(null, info.version);
});
child.stdin.write(
  JSON.stringify({
    jsonrpc: "2.0",
    id: 1,
    method: "initialize",
    params: {
      protocolVersion: "2024-11-05",
      capabilities: {},
      clientInfo: { name: "lvk-version-probe", version: "1" },
    },
  }) + "\n",
);
