import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import { expect, test } from "@playwright/test";

test("Codex user approval policy saves through Rust and survives page reload", async ({
  page,
}) => {
  test.setTimeout(240000);
  const binary = process.env.LVK_APPROVAL_TEST_BINARY;
  const child = spawn(
    binary ?? "node",
    binary
      ? [
          "agent_settings::tests::approval_policy_browser_bridge",
          "--exact",
          "--ignored",
          "--nocapture",
        ]
      : [
          "scripts/dev-cargo.mjs",
          "test",
          "-p",
          "executors",
          "approval_policy_browser_bridge",
          "--",
          "--ignored",
          "--nocapture",
        ],
    { cwd: process.cwd(), stdio: ["pipe", "pipe", "pipe"] },
  );
  let diagnostics = "";
  child.stderr.on("data", (chunk) => {
    diagnostics += chunk;
  });
  const lines = createInterface({ input: child.stdout });
  const replies: string[] = [];
  let wake: (() => void) | undefined;
  lines.on("line", (line) => {
    const start = line.indexOf("APPROVAL_BRIDGE_");
    if (start >= 0) {
      replies.push(line.slice(start));
      wake?.();
    }
  });
  const next = async () => {
    while (!replies.length)
      await new Promise<void>((resolve, reject) => {
        const timeout = setTimeout(
          () => reject(new Error(`Rust bridge timed out: ${diagnostics}`)),
          60000,
        );
        wake = () => {
          clearTimeout(timeout);
          resolve();
        };
      });
    return replies.shift()!;
  };
  let pending = Promise.resolve();
  const rpc = (path: string, body: unknown = null) => {
    const result = pending.then(async () => {
      child.stdin.write(JSON.stringify({ path, body }) + "\n");
      return JSON.parse((await next()).replace("APPROVAL_BRIDGE_REPLY ", ""));
    });
    pending = result.then(() => undefined);
    return result;
  };
  try {
    expect(await next()).toBe("APPROVAL_BRIDGE_READY");
    await page.route("**/api/agent-settings**", async (route) => {
      const request = route.request();
      const result = await rpc(
        new URL(request.url()).pathname,
        request.postDataJSON(),
      );
      await route.fulfill({ json: result });
    });
    const open = async () => {
      await page.goto("/?section=agents&lang=en&approvalBridge=true");
      await page
        .getByRole("button", { name: "設定を開く", exact: true })
        .click();
      await page
        .getByTestId("agent-settings-panel")
        .getByRole("tab", { name: /Permissions/ })
        .click();
    };
    await open();
    for (const value of ["on-request", "never", null]) {
      const panel = page.getByTestId("agent-settings-panel");
      const select = panel.getByRole("combobox", {
        name: "Approval policy",
        exact: true,
      });
      if (value) await select.selectOption(JSON.stringify(value));
      else
        await panel
          .getByRole("button", { name: "Restore inheritance", exact: true })
          .first()
          .click();
      await panel.getByRole("button", { name: "Save", exact: true }).click();
      await page
        .getByRole("button", { name: "Confirm apply", exact: true })
        .click();
      await expect(panel.getByRole("status")).toBeVisible();
      const verified = await rpc("/fixture/verify");
      expect(verified.data).toEqual({
        approval_policy: value,
        comment_preserved: true,
      });
      await open();
      await expect(
        page.getByRole("combobox", { name: "Approval policy", exact: true }),
      ).toHaveValue(value ? JSON.stringify(value) : "");
    }
  } finally {
    child.stdin.end();
    const code =
      child.exitCode ??
      (await new Promise((resolve) => child.once("exit", resolve)));
    lines.close();
    expect(code, diagnostics).toBe(0);
  }
});
