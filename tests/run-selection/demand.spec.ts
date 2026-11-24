import { test, expect } from "@playwright/test";

test("Claude selected-only discovery: loading, reverse responses, failure and reset", async ({
  page,
}, info) => {
  await page.route("**/api/**", (r) => r.abort());
  const calls: string[] = [];
  const replies: Array<() => void> = [];
  await page.routeWebSocket("**/api/agents/discovered-options/ws?**", (ws) => {
    const model = new URL(ws.url()).searchParams.get("selected_model")!;
    calls.push(model);
    replies.push(() => {
      ws.send(
        JSON.stringify({
          JsonPatch: [
            {
              op: "add",
              path: "/options",
              value: {
                model_selector: {
                  models: [],
                  providers: [],
                  agents: [],
                  permissions: [],
                  run_defaults: {
                    model,
                    reasoning: model === "beta" ? "low" : null,
                  },
                },
                loading_models: false,
              },
            },
          ],
        }),
      );
      ws.send(JSON.stringify({ Ready: true }));
      ws.send(JSON.stringify({ finished: true }));
    });
  });
  const started = Date.now();
  await page.goto("/?scenario=claude-demand");
  const picker = page.getByTestId("picker");
  await expect(picker).toContainText("Alpha");
  await expect(picker).toContainText("High");
  console.log(
    `initial usable browser=${Date.now() - started}ms, selected queries=${calls.length}`,
  );
  expect(calls).toEqual([]);
  const select = async (name: string) => {
    await picker
      .getByRole("button")
      .filter({ hasText: /Alpha|Beta|Gamma/ })
      .click();
    await page.getByRole("button", { name, exact: true }).click();
    await page.keyboard.press("Escape");
  };
  await select("Beta");
  await expect.poll(() => calls.length).toBe(1);
  await expect(picker).toContainText("読み込み");
  await expect(picker.locator(".text-brand")).toHaveText(["Beta"]);
  await select("Gamma");
  await expect.poll(() => calls.length).toBe(2);
  replies[1]();
  await expect(picker).toContainText("未確認");
  await expect(picker).toContainText("Gamma");
  replies[0](); // obsolete Beta reply must not change Gamma
  await expect(picker).not.toContainText("Low");
  await expect(picker.locator(".text-brand")).toHaveText(["Gamma"]);
  await select("Beta");
  await expect.poll(() => calls.length).toBe(3);
  await select("Alpha");
  replies[2](); // cleared selection must not reappear
  await expect(picker).toContainText("Alpha");
  await expect(picker).toContainText("High");
  await expect(picker.locator(".text-brand")).toHaveCount(0);
  await select("Beta");
  await expect.poll(() => calls.length).toBe(4);
  replies[3]();
  await expect(picker).toContainText("Low");
  await expect(picker.locator(".text-brand")).toHaveText(["Beta"]);
  await page.getByText("送信値を確認").click();
  const sent = JSON.parse(await page.getByTestId("sent").innerText());
  expect(sent.model_id).toBe("beta");
  expect(sent.reasoning_id ?? null).toBeNull();
  await page.screenshot({ path: info.outputPath("selected-beta.png") });
  await page.getByText("Run完了").click();
  await expect.poll(() => calls.length).toBe(5);
  replies[4]();
  await expect(picker).toContainText("Low");
  expect(calls).toEqual(["beta", "gamma", "beta", "beta", "beta"]);
});
