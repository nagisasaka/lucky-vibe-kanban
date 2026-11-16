import { expect, test } from "@playwright/test";
import { createHash, randomUUID } from "node:crypto";
import { mkdirSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

function responses() {
  return readdirSync(process.env.LVK_APPROVAL_FIXTURE_TRACE!)
    .flatMap((name) =>
      readFileSync(join(process.env.LVK_APPROVAL_FIXTURE_TRACE!, name), "utf8")
        .split("\n")
        .filter(Boolean)
        .map((line) => JSON.parse(line)),
    )
    .filter(
      (frame) =>
        frame.direction === "client_to_provider" &&
        frame.value.type === "control_response",
    )
    .map((frame) => frame.value.response);
}

test("Claude uses the common approval UI and returns decisions to the original request", async ({
  page,
  request,
}) => {
  const access = await (await request.get("/api/access/status")).json();
  const headers = {
    "x-lvk-csrf": access.csrf,
    Origin: process.env.LVK_E2E_BASE_URL!,
  };
  const api = async (
    path: string,
    data?: unknown,
    method = data === undefined ? "GET" : "POST",
  ) => {
    const response = await request.fetch(path, { method, data, headers });
    const body = await response.json();
    expect(response.ok(), `${path}: ${JSON.stringify(body)}`).toBeTruthy();
    expect(body.success, JSON.stringify(body)).toBeTruthy();
    return body.data;
  };
  const info = await api("/api/info");
  await api(
    "/api/config",
    {
      ...info.config,
      disclaimer_acknowledged: true,
      onboarding_acknowledged: true,
      analytics_enabled: false,
      show_release_notes: false,
    },
    "PUT",
  );
  const terminal = new Set([
    "succeeded",
    "failed",
    "cancelled",
    "crashed",
    "audit_failed",
  ]);
  for (const [kind, action] of [
    ["command", "accept"],
    ["file", "accept"],
    ["command", "deny"],
    ["file", "deny"],
    ["command", "cancel"],
    ["parallel", "accept"],
  ]) {
    const directory = join(process.env.LVK_SUPERVISED_TEST_ROOT!, randomUUID());
    mkdirSync(directory);
    const before = responses().length;
    const count = kind === "parallel" ? 2 : 1;
    const created = await api("/api/workspaces/start", {
      mode: "direct_folder",
      name: `Claude ${kind} ${action}`,
      repos: [],
      directory_path: directory,
      linked_issue: null,
      attachment_ids: [],
      executor_config: {
        executor: "CLAUDE_CODE",
        execution_mode: "code",
        permission_policy: "SUPERVISED",
      },
      prompt:
        kind === "parallel"
          ? "FIXTURE_PARALLEL"
          : kind === "file"
            ? "FIXTURE_FILE"
            : "FIXTURE_COMMAND",
    });
    const run = created.agent_run.agent_run_id;
    const events = async () =>
      (await api(`/api/agent-runs/${run}/events`)).events;
    const state = async () => api(`/api/agent-runs/${run}`);
    try {
      await expect
        .poll(
          async () =>
            (await events()).filter(
              (e: any) => e.payload.type === "approval_requested",
            ).length,
        )
        .toBe(count);
      const pending = (await events()).filter(
        (e: any) => e.payload.type === "approval_requested",
      );
      expect(responses()).toHaveLength(before);
      await page.goto(`/workspaces/${created.workspace.id}`);
      await page.keyboard.press("Escape");
      const approve = page
        .getByRole("button", { name: /^Approve$|^承認$/ })
        .last();
      await expect(approve).toBeEnabled();
      await page.reload();
      await expect(approve).toBeEnabled();
      expect(responses()).toHaveLength(before);
      if (kind !== "parallel") {
        await expect(
          page
            .getByText(
              kind === "command" ? "echo CLAUDE_APPROVAL" : "fixture content",
              { exact: false },
            )
            .first(),
        ).toBeVisible();
        await page.screenshot({
          path: test.info().outputPath(`claude-pending-${kind}-${action}.png`),
        });
      }
      if (action === "accept") {
        await approve.click();
        if (kind === "parallel") {
          await expect
            .poll(
              async () =>
                (await events()).filter(
                  (e: any) => e.payload.type === "approval_resolved",
                ).length,
            )
            .toBe(1);
          expect((await state()).status).toBe("awaiting_approval");
          await expect(approve).toBeEnabled();
          await approve.click();
        }
      } else if (action === "deny") {
        await page
          .getByRole("button", { name: /^Deny$|^拒否$/ })
          .last()
          .click();
      } else {
        await page
          .getByRole("button", { name: /^Stop$|^停止$/ })
          .last()
          .click();
      }
      await expect
        .poll(async () => terminal.has((await state()).status))
        .toBe(true);
      const history = await events();
      expect(
        history.filter((e: any) => e.payload.type === "approval_requested"),
      ).toHaveLength(count);
      expect(
        history.filter((e: any) => e.payload.type === "approval_resolved"),
      ).toHaveLength(count);
      const native = responses().slice(before);
      // Cancellation may close the transport before a denial can be written.
      if (action === "cancel") {
        expect(native.every((r) => r.response?.behavior !== "allow")).toBe(
          true,
        );
      } else {
        expect(native).toHaveLength(count);
        expect(new Set(native.map((r) => r.request_id))).toEqual(
          new Set(
            kind === "parallel"
              ? ["claude-native-command", "claude-native-file"]
              : [`claude-native-${kind}`],
          ),
        );
        for (const response of native) {
          expect(response.response.behavior).toBe(
            action === "accept" ? "allow" : "deny",
          );
          expect(response.response.updatedPermissions).toBeUndefined();
        }
      }
      const late = await request.post(`/api/agent-runs/${run}/approval`, {
        headers,
        data: {
          command_id: randomUUID(),
          idempotency_key: randomUUID(),
          correlation_id: randomUUID(),
          created_at: new Date().toISOString(),
          approval_id: pending[0].payload.data.approval_id,
          approved: true,
        },
      });
      expect(late.ok()).toBe(false);
      expect(responses().slice(before)).toEqual(native);
      const finalState = await state();
      const session = finalState.session_id;
      const auditDir = join(
        process.env.LVK_INSTANCE_DIR!,
        "data/runtime/native-audit/v1/sessions",
        session.slice(0, 2),
        session,
        "agent-runs",
        run,
        "attempts",
        finalState.last_run_attempt_id,
      );
      expect(
        JSON.parse(readFileSync(join(auditDir, "manifest.json"), "utf8"))
          .integrity_status,
      ).toBe("complete");
      const frames = readFileSync(join(auditDir, "frames.jsonl"), "utf8")
        .trim()
        .split("\n")
        .map((line) => JSON.parse(line));
      const decoded = frames.flatMap((frame) => {
        try {
          return [
            JSON.parse(
              Buffer.from(frame.payload_base64, "base64").toString("utf8"),
            ),
          ];
        } catch {
          return [];
        }
      });
      const sent = decoded
        .filter((v) => v.type === "approval_protocol_response")
        .map((v) => v.response.response);
      const nativeRequests = decoded.filter(
        (v) =>
          v.type === "control_request" && v.request?.subtype === "can_use_tool",
      );
      expect(nativeRequests).toHaveLength(count);
      for (const approval of pending) {
        const original = nativeRequests.find(
          (v) => v.request.tool_use_id === approval.payload.data.tool_call_id,
        );
        expect(original).toBeDefined();
        expect(original.request.decision_reason).toBe(
          "Isolated approval fixture",
        );
        expect(original.request.tool_name).toBe(
          approval.payload.data.tool_name,
        );
        expect(approval.agent_run_id).toBe(run);
      }
      const byId = (a: any, b: any) =>
        String(a.request_id).localeCompare(String(b.request_id));
      expect([...sent].sort(byId)).toEqual([...native].sort(byId));
      for (const event of history.filter((e: any) =>
        ["approval_requested", "approval_resolved"].includes(e.payload.type),
      )) {
        expect(event.native_refs).toHaveLength(1);
        const reference = event.native_refs[0];
        expect(reference.stream_id).toBe(finalState.last_run_attempt_id);
        const frame = frames.find((f) => f.sequence === reference.sequence);
        expect(
          createHash("sha256")
            .update(Buffer.from(frame.payload_base64, "base64"))
            .digest("hex"),
        ).toBe(reference.checksum);
      }
      console.log(
        JSON.stringify({
          kind,
          action,
          run,
          status: (await state()).status,
          native,
        }),
      );
    } finally {
      console.log(
        JSON.stringify({
          diagnostic: {
            kind,
            action,
            state: await state(),
            events: (await events()).map((e: any) => e.payload.type),
          },
        }),
      );
      if (!terminal.has((await state()).status)) {
        await api(`/api/agent-runs/${run}/cancel`, {
          command_id: randomUUID(),
          idempotency_key: randomUUID(),
          correlation_id: randomUUID(),
          created_at: new Date().toISOString(),
          reason: "Fixture cleanup",
        });
      }
    }
  }
});
