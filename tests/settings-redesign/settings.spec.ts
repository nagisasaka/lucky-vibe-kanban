import { expect, test, type Page } from "@playwright/test";

async function open(page: Page, query = "") {
  await page.goto("/" + query);
  await page.getByRole("button", { name: "設定を開く", exact: true }).click();
  await expect(
    page.getByRole("dialog", {
      name: query.includes("lang=en") ? "Settings" : "設定",
      exact: true,
    }),
  ).toBeVisible();
}

test("grouped navigation, storage scope, notifications and unchanged configuration values", async ({
  page,
}) => {
  await open(page, "?section=execution");
  await expect(
    page.getByRole("textbox").filter({ hasText: "fixture" }),
  ).toHaveCount(0);
  await expect(page.locator('input[value="fixture"]')).toBeVisible();
  await expect(
    page
      .getByText(
        "保存先：LVKインスタンス共通（config.json） · 下の保存ボタンで反映",
      )
      .first(),
  ).toBeVisible();
  await page
    .getByRole("navigation")
    .getByRole("button", { name: "通知", exact: true })
    .click();
  await expect(page.getByText("Web Push", { exact: true })).toBeVisible();
  await expect(page.locator('input[value="fixture"]')).toHaveCount(0);
  const sound = page.locator("#sound-enabled");
  await expect(sound).not.toBeChecked();
  await sound.click();
  await page.getByRole("button", { name: "保存", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "保存", exact: true }),
  ).toHaveCount(0);
  await sound.click();
  await page.getByRole("button", { name: "破棄", exact: true }).click();
  await expect(sound).toBeChecked();
  await page.screenshot({
    path: "test-results/settings-redesign/desktop-notifications.png",
  });
});

test("unsaved edits survive cancelled navigation, discard on confirmed navigation and protect close", async ({
  page,
}) => {
  await open(page, "?section=mcp-integration");
  const files = page.locator("#runtime-files");
  await files.click();
  await page
    .getByRole("navigation")
    .getByRole("button", { name: "通知", exact: true })
    .click();
  await page.getByRole("button", { name: "編集を続ける", exact: true }).click();
  await expect(files).not.toBeChecked();
  await page
    .getByRole("navigation")
    .getByRole("button", { name: "通知", exact: true })
    .click();
  await page
    .getByRole("button", { name: "変更を破棄", exact: true })
    .last()
    .click();
  await expect(page.getByText("Web Push", { exact: true })).toBeVisible();
  await page
    .getByRole("navigation")
    .getByRole("button", { name: "外部AI → LVK", exact: true })
    .click();
  await expect(files).toBeChecked();
  await files.click();
  await expect(page.getByRole("status")).toContainText("未保存");
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "編集を続ける", exact: true }).click();
  await expect(files).not.toBeChecked();
});

for (const width of [360, 390])
  test(`mobile ${width}: list, back, dirty retention and no horizontal overflow`, async ({
    page,
  }) => {
    await page.setViewportSize({ width, height: 780 });
    await open(page);
    await page
      .getByRole("navigation")
      .getByRole("button", { name: "外部AI → LVK", exact: true })
      .click();
    const files = page.locator("#runtime-files");
    await files.click();
    // Back to the list keeps the form mounted.
    await page
      .getByRole("dialog", { name: "設定", exact: true })
      .getByRole("button", { name: "設定", exact: true })
      .click();
    await page
      .getByRole("navigation")
      .getByRole("button", { name: "外部AI → LVK", exact: true })
      .click();
    await expect(files).not.toBeChecked();
    await expect(
      page.getByRole("button", { name: "保存", exact: true }),
    ).toBeInViewport();
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= innerWidth,
      ),
    ).toBe(true);
    await page.screenshot({
      path: `test-results/settings-redesign/mobile-${width}.png`,
    });
  });

for (const auth of ["basic", "auth0", "local"])
  test(`administrator management is embedded for ${auth}; secrets remain inputs`, async ({
    page,
  }) => {
    await open(page, `?auth=${auth}&section=access`);
    await expect(
      page
        .getByRole("navigation")
        .getByRole("button", { name: "認証・メンバー", exact: true }),
    ).toBeVisible();
    if (auth === "local")
      await expect(
        page.getByLabel("セットアップキー", { exact: true }),
      ).toHaveAttribute("type", "password");
    else
      await expect(
        page.getByRole("heading", { name: "LVK 認証とメンバー管理" }),
      ).toBeVisible();
    await page.screenshot({
      path: `test-results/settings-redesign/access-${auth}.png`,
    });
  });

test("member has no management navigation; explicit access falls back safely", async ({
  page,
}) => {
  await open(page, "?role=member&section=access");
  await expect(
    page
      .getByRole("navigation")
      .getByRole("button", { name: "認証・メンバー", exact: true }),
  ).toHaveCount(0);
  await expect(page.getByText("LVK 認証とメンバー管理")).toHaveCount(0);
  expect(
    await page.evaluate(() =>
      (
        window as unknown as { __settingsFixtureCalls: string[] }
      ).__settingsFixtureCalls.filter((path) =>
        /\/api\/access\/(configuration|members|audit)/.test(path),
      ),
    ),
  ).toEqual([]);
});

test("legacy management route and locked initial setup remain independent", async ({
  page,
}) => {
  await page.goto("/?access=settings");
  await expect(
    page.getByRole("heading", { name: "LVK 認証とメンバー管理" }),
  ).toBeVisible();
  await page.goto("/?auth=locked");
  await expect(
    page.getByLabel("セットアップキー", { exact: true }),
  ).toHaveAttribute("type", "password");
  await expect(page.getByRole("navigation")).toHaveCount(0);
});

test("English, dark theme and desktop keyboard focus", async ({ page }) => {
  await open(page, "?lang=en&theme=dark&section=mcp-integration");
  await expect(
    page
      .getByRole("navigation")
      .getByRole("button", { name: "External AI → LVK", exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Close", exact: true }).focus();
  await page.keyboard.press("Tab");
  expect(
    await page.evaluate(
      () => document.activeElement?.closest('[role="dialog"]') !== null,
    ),
  ).toBe(true);
  await page.screenshot({
    path: "test-results/settings-redesign/english-dark.png",
  });
});

test("authentication drafts revert cleanly and recovery remains reachable", async ({
  page,
}) => {
  await open(page, "?section=access");
  const username = page.getByLabel("ユーザー名", { exact: true });
  await username.fill("changed-demo");
  await expect(page.getByRole("status")).toContainText("未保存");
  await username.fill("demo");
  await expect(page.getByRole("status")).toHaveCount(0);
  await expect(
    page.getByRole("link", { name: "管理者向けの認証設定復旧" }),
  ).toHaveAttribute("href", "/?access=recovery");
  await page
    .getByRole("navigation")
    .getByRole("button", { name: "通知", exact: true })
    .click();
  await expect(
    page.getByRole("button", { name: "編集を続ける", exact: true }),
  ).toHaveCount(0);
});

test("every legacy settings destination remains navigable without render errors", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => {
    errors.push(error.message);
    console.log(error.message);
  });
  for (const section of [
    "general",
    "execution",
    "notifications",
    "agents",
    "agent-tools",
    "repos",
    "remote-projects",
    "mcp",
    "mcp-integration",
    "resources",
  ]) {
    console.log("Checking section", section);
    await open(page, "?section=" + section);
    await expect(
      page.getByRole("dialog", { name: "設定", exact: true }),
    ).toBeVisible();
    await page.getByRole("button", { name: "閉じる", exact: true }).click();
  }
  expect(errors).toEqual([]);
});

test("save failure preserves the notification draft and leave protection", async ({
  page,
}) => {
  await open(page, "?section=notifications&failSave=true");
  await page.locator("#sound-enabled").click();
  await page.getByRole("button", { name: "保存", exact: true }).click();
  await expect(
    page.getByText("Fixture save rejected", { exact: true }),
  ).toBeVisible();
  await expect(page.locator("#sound-enabled")).toBeChecked();
  await expect(
    page.getByRole("button", { name: "保存", exact: true }),
  ).toBeVisible();
  await page
    .getByRole("navigation")
    .getByRole("button", { name: "作業の既定値", exact: true })
    .click();
  await expect(
    page.getByRole("button", { name: "編集を続ける", exact: true }),
  ).toBeVisible();
});

test("audit summary remains in keyboard order", async ({ page }) => {
  await open(page, "?section=access&auth=auth0");
  const summary = page
    .locator("summary")
    .filter({ hasText: "最近の認証・操作監査" });
  await summary.focus();
  await page.keyboard.press("Shift+Tab");
  await page.keyboard.press("Tab");
  await expect(summary).toBeFocused();
});

for (const auth of ["basic", "auth0", "local"])
  test(`mobile management ${auth} stays within 360px`, async ({ page }) => {
    await page.setViewportSize({ width: 360, height: 780 });
    await open(page, "?section=access&auth=" + auth);
    if (auth === "local")
      await expect(
        page.getByLabel("セットアップキー", { exact: true }),
      ).toBeVisible();
    else
      await expect(
        page.getByRole("heading", { name: "LVK 認証とメンバー管理" }),
      ).toBeVisible();
    expect(
      await page
        .getByRole("dialog")
        .evaluate((el) =>
          [...el.querySelectorAll<HTMLElement>("*")]
            .filter(
              (node) =>
                node.clientWidth > 0 &&
                node.scrollWidth > node.clientWidth + 1 &&
                getComputedStyle(node).overflowX === "visible",
            )
            .map((node) => node.tagName + "." + node.className),
        ),
    ).toEqual([]);
    await page.screenshot({
      path: `test-results/settings-redesign/mobile-access-${auth}.png`,
    });
  });

test("updating stored ChatGPT credentials protects the draft and clears dirty after its own save", async ({
  page,
}) => {
  await open(page, "?section=access&auth=auth0");
  await expect(
    page.getByLabel("ChatGPT用Client ID", { exact: true }),
  ).toHaveValue("fixture-chat");
  await page.getByText("既存の接続情報を保存・更新", { exact: true }).click();
  await page
    .getByLabel("保存するChatGPT Client ID", { exact: true })
    .fill("updated-fixture-chat");
  await page
    .getByLabel("保存するChatGPT Client Secret", { exact: true })
    .fill("sample-only-secret");
  await expect(page.getByRole("status")).toContainText("未保存");
  await page
    .getByRole("navigation")
    .getByRole("button", { name: "通知", exact: true })
    .click();
  await page.getByRole("button", { name: "編集を続ける", exact: true }).click();
  await expect(
    page.getByLabel("保存するChatGPT Client Secret", { exact: true }),
  ).toHaveAttribute("type", "password");
  await page
    .getByRole("button", { name: "ChatGPT接続情報を保存", exact: true })
    .click();
  await expect(
    page.getByLabel("保存するChatGPT Client Secret", { exact: true }),
  ).toHaveValue("");
  await expect(
    page.getByText("• 未保存の変更があります", { exact: true }),
  ).toHaveCount(0);
  await page
    .getByRole("navigation")
    .getByRole("button", { name: "通知", exact: true })
    .click();
  await expect(
    page.getByRole("button", { name: "編集を続ける", exact: true }),
  ).toHaveCount(0);
});

test("a successful authentication handoff does not warn about an already committed draft", async ({
  page,
}) => {
  const dialogs: string[] = [];
  page.on("dialog", (dialog) => {
    dialogs.push(dialog.type());
    void dialog.dismiss();
  });
  await open(page, "?section=access&allowHandoff=true");
  await page
    .getByLabel("ユーザー名", { exact: true })
    .fill("saved-fixture-user");
  await page
    .getByLabel("新しいパスワード（12文字以上）", { exact: true })
    .fill("sample-only-password");
  await expect(page.getByRole("status")).toContainText("未保存");
  await page
    .getByRole("button", { name: "Basicを設定して再ログイン", exact: true })
    .click();
  await expect(page).toHaveURL("http://127.0.0.1:4196/");
  expect(dialogs).toEqual([]);
});

for (const failSave of [false, true]) {
  test(`merged OS event controls preserve save scope and drafts (failure=${failSave})`, async ({
    page,
  }) => {
    await open(page, `?section=notifications&failSave=${failSave}`);
    const event = page.locator("#os-push-event-1");
    await expect(event).toBeChecked();
    await expect(page.locator('[id^="os-push-event-"]')).toHaveCount(4);
    const persisted = () =>
      page.evaluate(async () => {
        const response = await fetch("/api/info");
        return (await response.json()).data.config;
      });
    const before = await persisted();
    await event.click();
    await expect(event).not.toBeChecked();
    expect(await persisted()).toEqual(before);
    await page.getByRole("button", { name: "破棄", exact: true }).click();
    await expect(event).toBeChecked();
    await event.click();
    await page.getByRole("button", { name: "保存", exact: true }).click();
    if (failSave) {
      await expect(
        page.getByText("Fixture save rejected", { exact: true }),
      ).toBeVisible();
      await expect(event).not.toBeChecked();
      expect(await persisted()).toEqual(before);
      await page.getByRole("button", { name: "破棄", exact: true }).click();
      await expect(event).toBeChecked();
    } else {
      await expect(
        page.getByRole("button", { name: "保存", exact: true }),
      ).toHaveCount(0);
      expect(await persisted()).toEqual({
        ...before,
        notifications: { ...before.notifications, push_event_mask: 14 },
      });
      await page
        .getByRole("navigation")
        .getByRole("button", { name: "表示・操作とプライバシー", exact: true })
        .click();
      await expect(event).toHaveCount(0);
      await page
        .getByRole("navigation")
        .getByRole("button", { name: "通知", exact: true })
        .click();
      await expect(event).not.toBeChecked();
    }
  });
}
