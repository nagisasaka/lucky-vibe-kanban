import { expect } from "@playwright/test";
import { randomUUID } from "node:crypto";
import { join } from "node:path";

// Uses the built real app, isolated API/DB, and synthetic canonical streams.
// Follow-up HTTP is intercepted before it can launch a provider.
export async function verifyScrollFollow({
  page,
  session,
  extraRuns,
  evidence,
  report,
}) {
  await page.setViewportSize({ width: 1180, height: 800 });
  const scroller = page
    .locator(".overflow-y-auto")
    .filter({ has: page.locator("[data-row-index]") });
  const composer = page.locator('[contenteditable="true"]').last();
  const distance = () =>
    scroller.evaluate((el) => el.scrollHeight - el.clientHeight - el.scrollTop);
  const bottom = async (label) =>
    expect.poll(distance, { message: label, timeout: 7000 }).toBeLessThan(3);
  await bottom("initial load");
  let held;
  await page.route(`**/api/sessions/${session}/follow-up`, (route) => {
    held = route;
  });
  const identities = [];
  for (let round = 1; round <= 3; round++) {
    await scroller.hover();
    await page.mouse.wheel(0, -900);
    await expect(
      page.getByRole("button", { name: "Scroll to bottom", exact: true }),
    ).toBeVisible();
    const prompt = `通常ユーザー送信 ${round}`;
    held = null;
    await composer.fill(prompt);
    await composer.press("Enter");
    await expect.poll(() => Boolean(held)).toBe(true);
    const localRow = page
      .locator("[data-semantic-key]")
      .filter({ hasText: prompt });
    await expect(localRow).toHaveCount(1);
    const identity = await localRow.getAttribute("data-semantic-key");
    await localRow.evaluate((el) => {
      el.dataset.identityProbe = "retained";
    });
    await bottom("own send must immediately leave history");
    const id = randomUUID();
    let seq = 0,
      ws;
    let replayReady = false;
    let state = {
      state_schema_version: 1,
      reducer_version: 1,
      session_id: session,
      agent_run_id: id,
      turn_id: id,
      status: "running",
      projection_status: "current",
      last_run_attempt_id: id,
      last_run_attempt_number: 1,
      last_event_sequence: 0,
      unknown_event_count: 0,
      updated_at: new Date().toISOString(),
    };
    const summary = {
      agent_run_id: id,
      session_id: session,
      turn_id: id,
      state,
      created_at: new Date().toISOString(),
      completed_at: null,
      updated_at: state.updated_at,
    };
    const event = (payload) => ({
      schema_version: 1,
      payload_version: 1,
      event_id: randomUUID(),
      session_id: session,
      agent_run_id: id,
      turn_id: id,
      run_attempt_id: id,
      run_attempt_number: 1,
      sequence: ++seq,
      correlation_id: id,
      timestamp: new Date().toISOString(),
      native_refs: [],
      payload,
    });
    const events = [];
    const send = (payload) => {
      const next = event(payload);
      events.push(next);
      ws.send(
        JSON.stringify({
          type: "event",
          data: { event: next, replay: false },
        }),
      );
    };
    await page.routeWebSocket(`**/api/agent-runs/${id}/events/ws`, async (socket) => {
      replayReady = false;
      let closed = false;
      socket.onClose(() => { closed = true; });
      await new Promise(resolve => setTimeout(resolve, 50));
      if (closed) return;
      ws = socket;
      if (events.length === 0) {
      send({
        type: "message",
        data: {
          message: {
            message_id: `user-${round}`,
            role: "user",
            content: prompt,
          },
          final_output: false,
        },
      });
      send({ type: "lifecycle_changed", data: { status: "running" } });
      } else {
        for (const event of events) {
          if (closed) return;
          socket.send(JSON.stringify({type:'event', data:{event, replay:true}}));
          await new Promise(resolve => setTimeout(resolve, 2));
        }
      }
      if (closed) return;
      socket.send(
        JSON.stringify({
          type: "ready",
          data: { state: { ...state, last_event_sequence: seq }, cursor: null },
        }),
      );
      replayReady = true;
    });
    extraRuns.push(summary);
    // Delay ACK while the local row is already visible. Discovery may win ACK.
    await page.waitForTimeout(200);
    let readingBeforeAck;
    if (round === 3) {
      await scroller.hover();
      await page.mouse.wheel(0, -200);
      await page.waitForTimeout(100);
      readingBeforeAck = await scroller.evaluate((el) => {
        const top = el.getBoundingClientRect().top;
        const row = [...el.querySelectorAll("[data-semantic-key]")].find(
          (row) => row.getBoundingClientRect().bottom > top,
        );
        return {
          key: row.dataset.semanticKey,
          top: row.getBoundingClientRect().top - top,
        };
      });
    }
    await held.fulfill({
      json: { success: true, data: { agent_run_id: id, state } },
    });
    await expect.poll(() => Boolean(ws) && replayReady, { timeout: 10000 }).toBe(true);
    if (round === 3) {
      await page.waitForTimeout(250);
      const after = await scroller.evaluate((el, key) => {
        const row = [...el.querySelectorAll("[data-semantic-key]")].find(
          (row) => row.dataset.semanticKey === key,
        );
        return row
          ? row.getBoundingClientRect().top - el.getBoundingClientRect().top
          : null;
      }, readingBeforeAck.key);
      report.delayedAckAnchor = {before: readingBeforeAck, after};
      expect(
        after,
        "reading row must remain mounted through ACK",
      ).not.toBeNull();
      expect(
        Math.abs(after - readingBeforeAck.top),
        "delayed ACK must respect visual reading anchor, including virtual head changes",
      ).toBeLessThan(3);
      await page
        .getByRole("button", { name: "Scroll to bottom", exact: true })
        .click();
      await bottom("resume after delayed ACK");
    }
    const response = (n) =>
      Array.from(
        { length: n },
        (_, i) => `${i + 1}. ROUND-${round}-LINE-${i + 1} 表示確認の文章です。`,
      ).join("\n");
    const publish = (n) =>
      send({
        type: "message",
        data: {
          message: {
            message_id: `answer-${round}`,
            role: "assistant",
            content: response(n),
          },
          final_output: false,
        },
      });
    for (const lines of [6, 12, 20, 24]) {
      publish(lines);
      if (round === 1 && lines === 20) {
        send({
          type: "tool_call",
          data: {
            tool_call_id: "scroll-tool",
            tool_name: "Shell",
            status: "succeeded",
            arguments: { command: 'printf "synthetic output"' },
            result: {
              aggregatedOutput: "Synthetic tool result\nSecond result line",
              exitCode: 0,
            },
          },
        });
      }
      await page.waitForTimeout(100);
      await bottom(`round${round} streamed ${lines} lines`);
      if (round === 2 && lines === 6) {
        // Even a small deliberate move inside the old near-bottom threshold
        // must suspend following until the user explicitly resumes it.
        await scroller.hover();
        await page.mouse.wheel(0, -40);
        await page.waitForTimeout(80);
        const before = await scroller.evaluate((el) => el.scrollTop);
        publish(10);
        await page.waitForTimeout(200);
        expect(
          Math.abs((await scroller.evaluate((el) => el.scrollTop)) - before),
        ).toBeLessThan(3);
        const down = page.getByRole("button", {
          name: "Scroll to bottom",
          exact: true,
        });
        await expect(down).toBeVisible();
        await down.click();
        await bottom("button resumes follow");
      }
    }
    send({ type: "lifecycle_changed", data: { status: "succeeded" } });
    state = {
      ...state,
      status: "succeeded",
      updated_at: new Date().toISOString(),
      last_event_sequence: seq,
    };
    summary.state = state;
    summary.completed_at = state.updated_at;
    summary.updated_at = state.updated_at;
    ws.send(JSON.stringify({ type: "state", data: { state, cursor: null } }));
    await bottom(`round${round} terminal/footer`);
    await expect
      .poll(() =>
        page.getByTestId("run-elapsed").last().locator("time").count(),
      )
      .toBe(1);
    const line = page.getByText(new RegExp(`ROUND-${round}-LINE-24`));
    await expect(line).toBeVisible();
    const visible = await line.evaluate((el) => {
      const viewport = el.closest(".overflow-y-auto").getBoundingClientRect();
      const r = el.getBoundingClientRect();
      return r.top >= viewport.top - 1 && r.bottom <= viewport.bottom + 1;
    });
    expect(
      visible,
      "last response line must fit viewport, not just exist in DOM",
    ).toBe(true);
    const canonicalRow = page
      .locator("[data-semantic-key]")
      .filter({ hasText: prompt });
    identities.push({
      local: identity,
      canonical: await canonicalRow.getAttribute("data-semantic-key"),
      sameNode:
        (await canonicalRow.getAttribute("data-identity-probe")) === "retained",
    });
    await page.screenshot({
      path: join(evidence, `scroll-round-${round}.png`),
    });
    // Simulate an image/font/Markdown layout arriving after the final event.
    const answer = page
      .locator("[data-row-index]")
      .filter({ hasText: `ROUND-${round}-LINE-24` });
    await answer.evaluate((el) => {
      const delayed = document.createElement("div");
      delayed.dataset.delayedLayout = "true";
      delayed.style.height = "220px";
      el.append(delayed);
    });
    await bottom("delayed completed layout");
    await page.setViewportSize({
      width: round === 3 ? 760 : 1180,
      height: 640,
    });
    await bottom("viewport resize");
    await page.evaluate(() =>
      document
        .querySelectorAll("[data-delayed-layout]")
        .forEach((el) => el.remove()),
    );
    await page.setViewportSize({ width: 1180, height: 800 });
    await bottom("restored viewport");
    await expect(
      page.getByRole("button", { name: "Scroll to bottom", exact: true }),
    ).toHaveCount(0);
    await page.waitForTimeout(100);
  }
  report.scrollIdentities = identities;
  if (!process.env.LVK_CHAT_SCROLL_BASELINE)
    for (const identity of identities) {
      expect(identity.canonical).toBe(identity.local);
      expect(
        identity.sameNode,
        "ACK must not replace the submitted user row",
      ).toBe(true);
    }
  report.checks.push(
    "actual app: three own sends from history, 24-line Markdown responses, small manual scroll, bottom-button resume, terminal/footer, delayed DOM and viewport resize; local/canonical same row identity",
  );
}
