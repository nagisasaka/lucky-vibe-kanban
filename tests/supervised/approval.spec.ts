import { expect, test } from "@playwright/test";
import { createHash, randomUUID } from "node:crypto";
import { mkdirSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

const identity = () => ({
  command_id: randomUUID(),
  idempotency_key: randomUUID(),
  correlation_id: randomUUID(),
  created_at: new Date().toISOString(),
});
const terminal = new Set([
  "succeeded",
  "failed",
  "cancelled",
  "crashed",
  "audit_failed",
]);
function nativeResponses() {
  return readdirSync(process.env.LVK_APPROVAL_FIXTURE_TRACE!)
    .flatMap((name) =>
      readFileSync(join(process.env.LVK_APPROVAL_FIXTURE_TRACE!, name), "utf8")
        .trim()
        .split("\n")
        .filter(Boolean)
        .map((line) => JSON.parse(line)),
    )
    .filter(
      (v) => v.direction === "client_to_provider" && v.value.result?.decision,
    )
    .map((v) => v.value);
}

test("real approval UI and backend wait, accept, deny, cancel, and preserve full access", async ({
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
      commit_reminder_enabled: false,
    },
    "PUT",
  );
  for (const [kind, action] of [
    ["command", "accept"],
    ["file", "accept"],
    ["command", "deny"],
    ["file", "deny"],
    ["command", "cancel"],
    ["file", "cancel"],
    ["command", "full-access"],
    ["file", "full-access"],
    ["parallel", "accept"],
  ]) {
    const directory = join(process.env.LVK_SUPERVISED_TEST_ROOT!, randomUUID());
    mkdirSync(directory);
    const before = nativeResponses().length;
    const count = kind === "parallel" ? 2 : 1;
    const created = await api("/api/workspaces/start", {
      mode: "direct_folder",
      name: `LOCAL38 ${kind} ${action}`,
      repos: [],
      directory_path: directory,
      linked_issue: null,
      attachment_ids: [],
      executor_config: {
        executor: "CODEX",
        execution_mode: "code",
        permission_policy: action === "full-access" ? "AUTO" : "SUPERVISED",
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
      if (action !== "full-access") {
        await expect
          .poll(
            async () =>
              (await events()).filter(
                (e: any) => e.payload.type === "approval_requested",
              ).length,
          )
          .toBe(count);
        const pending = (await events()).find(
          (e: any) => e.payload.type === "approval_requested",
        ).payload.data;
        expect(nativeResponses()).toHaveLength(before);
        await page.goto(`/workspaces/${created.workspace.id}`);
        await page.keyboard.press("Escape");
        const approve = page
          .getByRole("button", { name: /^Approve$|^承認$/ })
          .last();
        await expect(approve).toBeVisible();
        await expect(approve).toBeEnabled();
        // Navigation/reload and idle rendering must not approve a request.
        await page.reload();
        await expect(approve).toBeEnabled();
        expect(nativeResponses()).toHaveLength(before);
        if (action === "accept" && kind !== "parallel") {
          await expect(
            page
              .getByText(
                kind === "command"
                  ? '"command": "echo LOCAL38_APPROVAL"'
                  : '"diff": "+fixture"',
                { exact: false },
              )
              .first(),
          ).toBeVisible();
          await page.screenshot({
            path: test.info().outputPath(`pending-${kind}.png`),
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
        }
        if (action === "deny")
          await page
            .getByRole("button", { name: /^Deny$|^拒否$/ })
            .last()
            .click();
        if (action === "cancel")
          await page
            .getByRole("button", { name: /^Stop$|^停止$/ })
            .last()
            .click();
        await expect
          .poll(async () => terminal.has((await state()).status))
          .toBe(true);
        const late = await request.post(`/api/agent-runs/${run}/approval`, {
          headers,
          data: {
            ...identity(),
            approval_id: pending.approval_id,
            approved: true,
          },
        });
        // Commands may be accepted durably then rejected by the host worker.
        if (late.ok())
          await expect
            .poll(async () => nativeResponses().length)
            .toBe(before + (action === "cancel" ? 0 : count));
        const history = await events();
        expect(
          history.filter((e: any) => e.payload.type === "approval_requested"),
        ).toHaveLength(count);
        expect(
          history.filter((e: any) => e.payload.type === "approval_resolved")
            .length,
        ).toBe(count);
      } else {
        await expect
          .poll(async () => terminal.has((await state()).status))
          .toBe(true);
        expect(
          (await events()).filter(
            (e: any) => e.payload.type === "approval_requested",
          ),
        ).toHaveLength(0);
      }
      const responses = nativeResponses().slice(before);
      expect(responses).toHaveLength(action === "cancel" ? 0 : count);
      if (responses.length) {
        if (kind === "parallel")
          expect(new Set(responses.map((v) => v.id))).toEqual(
            new Set([701, "file-native-702"]),
          );
        else
          expect(responses[0].id).toBe(
            kind === "command" ? 701 : "file-native-702",
          );
        expect(responses[0].result.decision).toBe(
          action === "accept"
            ? "accept"
            : action === "full-access"
              ? "acceptForSession"
              : "cancel",
        );
      }
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
      const manifest = JSON.parse(
        readFileSync(join(auditDir, "manifest.json"), "utf8"),
      );
      expect(manifest.integrity_status).toBe("complete");
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
      const auditRequests = decoded.filter(
        (value) =>
          typeof value.method === "string" &&
          value.method.endsWith("/requestApproval"),
      );
      const auditResponses = decoded
        .filter((value) => value.type === "approval_protocol_response")
        .map((value) => value.response);
      expect(auditRequests).toHaveLength(count);
      const byId = (a: any, b: any) => String(a.id).localeCompare(String(b.id));
      expect([...auditResponses].sort(byId)).toEqual([...responses].sort(byId));
      const controls = (await events()).filter((e: any) =>
        ["approval_requested", "approval_resolved"].includes(e.payload.type),
      );
      for (const event of controls) {
        expect(event.native_refs).toHaveLength(1);
        const reference = event.native_refs[0];
        expect(reference.stream_id).toBe(finalState.last_run_attempt_id);
        const frame = frames.find((f) => f.sequence === reference.sequence);
        expect(
          createHash("sha256")
            .update(Buffer.from(frame.payload_base64, "base64"))
            .digest("hex"),
        ).toBe(reference.checksum);
        const payload = JSON.parse(
          Buffer.from(frame.payload_base64, "base64").toString("utf8"),
        );
        expect(payload.approval_id).toBe(event.payload.data.approval_id);
      }
      console.log(
        JSON.stringify({
          audit: {
            run,
            integrity: manifest.integrity_status,
            requests: auditRequests.map((v) => ({
              id: v.id,
              method: v.method,
            })),
            responses: auditResponses,
            verifiedNativeReferences: controls.length,
          },
        }),
      );
      console.log(
        JSON.stringify({
          kind,
          action,
          run,
          status: (await state()).status,
          responses,
        }),
      );
    } finally {
      console.log(
        JSON.stringify({
          diagnostic: {
            kind,
            action,
            status: (await state()).status,
            eventTypes: (await events()).map((e: any) => e.payload.type),
          },
        }),
      );
      if (!terminal.has((await state()).status))
        await api(`/api/agent-runs/${run}/cancel`, {
          ...identity(),
          reason: "Fixture cleanup",
        });
    }
  }
});
