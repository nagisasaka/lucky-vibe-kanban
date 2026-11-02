import { expectNoHorizontalOverflow, switchLanguage } from "./i18n-support";
import { expect, test, type Page } from "@playwright/test";

async function open(page: Page, language: string, extra = "") {
  await page.addInitScript(() =>
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: {
        writeText: async (value: string) => {
          Object.defineProperty(window, "__fixtureCopiedPath", {
            configurable: true,
            value,
          });
        },
      },
    }),
  );
  await page.goto(`/?view=agent-tools&lang=${language}${extra}`);
  await expect(
    page.getByRole("heading", {
      name: language === "ja" ? "エージェントツール" : "Agent Tools",
      exact: true,
    }),
  ).toBeVisible();
}
async function calls(page: Page) {
  return page.evaluate(
    () =>
      (
        window as unknown as {
          __agentToolsFixture: {
            calls: Array<{ path: string; body: Record<string, unknown> }>;
          };
        }
      ).__agentToolsFixture.calls,
  );
}

for (const language of ["en", "ja"] as const) {
  const ja = language === "ja";
  test(`Agent Tools ${language}: states, copy warning, raw identities and confirmed native read`, async ({
    page,
  }) => {
    await open(page, language);
    await expect(page.getByText("MCP_RAW", { exact: true })).toBeVisible();
    await expect(
      page.getByText(ja ? "有効" : "Enabled", { exact: true }),
    ).toBeVisible();
    await expect(
      page.getByText(ja ? "ユーザー" : "User", { exact: true }),
    ).toBeVisible();
    await expect(
      page.getByText(ja ? "1件" : "1 item", { exact: true }),
    ).toBeVisible();
    await expect(page.locator("body")).not.toContainText(
      "fixture-sensitive-value",
    );
    page.once("dialog", async (dialog) => {
      expect(dialog.message()).toContain(ja ? "認証情報" : "Credentials");
      await dialog.dismiss();
    });
    await page
      .getByRole("button", {
        name: ja ? "定義を読み込む" : "Read definition",
        exact: true,
      })
      .click();
    expect(await calls(page)).toHaveLength(0);
    page.once("dialog", async (dialog) => {
      expect(dialog.message()).toContain(ja ? "認証情報" : "Credentials");
      await dialog.accept();
    });
    await page
      .getByRole("button", {
        name: ja ? "定義を読み込む" : "Read definition",
        exact: true,
      })
      .click();
    await expect(page.locator("pre")).toContainText("fixture-sensitive-value");
    await expect(page.locator("pre")).toContainText(
      "fixture-private-native-command",
    );
    expect((await calls(page))[0].body).toMatchObject({
      expected_revision: "revision-1",
      confirmed_sensitive_read: true,
      target: { provider: "codex", scope: "user", name: "MCP_RAW" },
    });
    await page
      .getByRole("button", {
        name: ja ? "内容を隠す" : "Hide contents",
        exact: true,
      })
      .click();
    await expect(page.locator("pre")).toHaveCount(0);
    page.once("dialog", async (dialog) => {
      expect(dialog.message()).toContain("claude_code");
      await dialog.accept("claude_code");
    });
    await page
      .getByRole("button", { name: ja ? "コピー" : "Copy", exact: true })
      .click();
    await expect(
      page
        .getByRole("status")
        .filter({ hasText: /Provider-specific fields|プロバイダー固有/ }),
    ).toContainText(
      ja
        ? "プロバイダー固有の項目はコピー元に保持され"
        : "Provider-specific fields remain at the source",
    );
    const copy = (await calls(page)).find((call) =>
      call.path.endsWith("/copy"),
    )!;
    expect(copy.body).toMatchObject({
      source: { provider: "codex", scope: "user", name: "MCP_RAW" },
      expected_revision: "revision-1",
      target_provider: "claude_code",
      target_scope: "user",
      replace: false,
    });
    await switchLanguage(page, ja ? "en" : "ja");
    await expect(
      page
        .getByRole("status")
        .filter({ hasText: /Provider-specific fields|プロバイダー固有/ }),
    ).toContainText(
      ja
        ? "Provider-specific fields remain at the source"
        : "プロバイダー固有の項目はコピー元に保持され",
    );
    await page
      .getByRole("button", {
        name: ja ? "Copy path" : "保存場所をコピー",
        exact: true,
      })
      .click();
    await expect(page.getByRole("status")).toContainText("/fixture/raw-tool");
    expect(
      await page.evaluate(
        () =>
          (window as unknown as { __fixtureCopiedPath: string })
            .__fixtureCopiedPath,
      ),
    ).toBe("/fixture/raw-tool");
    await expect(page.getByRole("status")).toContainText(
      ja ? "Native path copied:" : "ネイティブの保存場所をコピーしました:",
    );
    await page.evaluate(() =>
      Object.defineProperty(navigator.clipboard, "writeText", {
        configurable: true,
        value: async () => {
          throw new Error("fixture private clipboard detail");
        },
      }),
    );
    await page
      .getByRole("button", {
        name: ja ? "Copy path" : "保存場所をコピー",
        exact: true,
      })
      .click();
    await expect(page.getByRole("alert")).toHaveText(
      ja
        ? "Unable to copy the native path. Allow clipboard access and try again."
        : "ネイティブの保存場所をコピーできませんでした。クリップボードへのアクセスを許可してから操作してください。",
    );
    await expect(page.getByRole("alert")).not.toContainText("fixture private");
    await page.setViewportSize({ width: 360, height: 900 });
    await expectNoHorizontalOverflow(page);
  });

  test(`Agent Tools ${language}: invalid MCP JSON, structured errors and locale switching`, async ({
    page,
  }) => {
    await open(page, language);
    page.once("dialog", async (dialog) => {
      expect(dialog.message()).toContain("preserve");
      expect(dialog.message()).toContain("replace");
      await dialog.accept("invalid fixture JSON");
    });
    await page
      .getByRole("button", { name: ja ? "編集" : "Edit", exact: true })
      .click();
    await expect(page.getByRole("alert")).toHaveText(
      ja
        ? "MCPのJSONが正しくありません。入力を確認してください。書き込みリクエストは送信していません。"
        : "Invalid MCP JSON. Check the input; no write was submitted.",
    );
    expect(await calls(page)).toHaveLength(0);
    await page.evaluate(() =>
      (
        window as unknown as {
          __agentToolsFixture: { setFailure: (code: string) => void };
        }
      ).__agentToolsFixture.setFailure("stale_revision"),
    );
    await page
      .getByRole("switch", {
        name: ja ? "MCP_RAW の有効・無効を切り替え" : "Toggle MCP_RAW",
        exact: true,
      })
      .click();
    await expect(page.getByRole("alert")).toHaveText(
      ja
        ? "ネイティブの内容が変更されました。編集する前に再読み込みしてください。"
        : "Native content changed. Refresh before editing.",
    );
    await expect(page.locator("body")).not.toContainText("fixture private");
    await switchLanguage(page, ja ? "en" : "ja");
    await expect(page.getByRole("alert")).toHaveText(
      ja
        ? "Native content changed. Refresh before editing."
        : "ネイティブの内容が変更されました。編集する前に再読み込みしてください。",
    );
    const toggle = (await calls(page))[0];
    expect(toggle.body).toMatchObject({
      expected_revision: "revision-1",
      enabled: false,
      target: { provider: "codex", scope: "user" },
    });
  });

  test(`Agent Tools ${language}: add uses canonical provider/scope values; Skill edits preserve raw text`, async ({
    page,
  }) => {
    await open(page, language);
    const projectPath = "/fixture/project 原文";
    await page
      .getByLabel(
        ja ? "プロジェクトのパス（任意）" : "Project path (optional)",
        { exact: true },
      )
      .fill(projectPath);
    await expect(
      page.getByRole("button", { name: ja ? "追加" : "Add", exact: true }),
    ).toBeDisabled();
    await page
      .getByRole("button", {
        name: ja ? "プロジェクトのツールを検出" : "Discover project tools",
        exact: true,
      })
      .click();
    await expect(
      page.getByRole("button", { name: ja ? "追加" : "Add", exact: true }),
    ).toBeEnabled();
    let step = 0;
    const addDialog = async (dialog: import("@playwright/test").Dialog) => {
      const values = ["codex", "project", "RAW_ADD"];
      if (step < 3) await dialog.accept(values[step++]);
      else {
        const definition = JSON.parse(dialog.defaultValue());
        definition.command = {
          type: "replace",
          data: { value: "fixture-command" },
        };
        step++;
        await dialog.accept(JSON.stringify(definition));
      }
    };
    page.on("dialog", addDialog);
    await page
      .getByRole("button", { name: ja ? "追加" : "Add", exact: true })
      .click();
    await expect(page.getByText("RAW_ADD", { exact: true })).toBeVisible();
    page.off("dialog", addDialog);
    expect(step).toBe(4);
    expect((await calls(page))[0].body).toMatchObject({
      target: {
        provider: "codex",
        scope: "project",
        project_path: projectPath,
        kind: "mcp_server",
        name: "RAW_ADD",
      },
      replace: false,
      expected_revision: null,
      definition: {
        type: "mcp_server",
        data: {
          command: { type: "replace", data: { value: "fixture-command" } },
        },
      },
    });
    await page
      .getByRole("tab", { name: ja ? "スキル" : "Skills", exact: true })
      .click();
    const editSkillDialog = async (
      dialog: import("@playwright/test").Dialog,
    ) => {
      if (dialog.type() === "confirm") await dialog.accept();
      else {
        expect(dialog.message()).toContain("SKILL.md");
        expect(dialog.defaultValue()).toBe("# Raw Skill 日本語");
        await dialog.accept("# Edited Skill 原文");
      }
    };
    page.on("dialog", editSkillDialog);
    await page
      .getByRole("button", { name: ja ? "編集" : "Edit", exact: true })
      .click();
    await expect
      .poll(
        async () =>
          (await calls(page)).filter((call) => call.path === "/api/agent-tools")
            .length,
      )
      .toBe(2);
    page.off("dialog", editSkillDialog);
    const update = (await calls(page)).at(-1)!;
    expect(update.body).toMatchObject({
      expected_revision: "skill-revision-1",
      target: {
        provider: "codex",
        scope: "user",
        kind: "skill",
        name: "SKILL_RAW",
      },
      definition: {
        type: "skill",
        data: {
          type: "replace_contract",
          data: { value: "# Edited Skill 原文" },
        },
      },
    });
  });

  test(`Agent Tools ${language}: loading, load failure and provider errors fail closed`, async ({
    page,
  }) => {
    await open(page, language, "&toolsState=loading");
    await expect(page.getByRole("status")).toContainText(
      ja
        ? "エージェントツールの一覧を読み込み中…"
        : "Loading Agent Tool inventory...",
    );
    await open(page, language, "&toolsState=error");
    await expect(page.getByRole("alert")).toHaveText(
      ja
        ? "ネイティブ設定が正しくありません。続行する前にネイティブファイルを確認してください。"
        : "Invalid native configuration. Inspect the native file before continuing.",
    );
    await expect(
      page.getByRole("button", { name: ja ? "追加" : "Add", exact: true }),
    ).toBeDisabled();
    await open(page, language, "&toolsState=error&toolsFailure=future_code");
    await expect(page.getByRole("alert")).toHaveText(
      ja
        ? "操作結果を確認できませんでした。別の変更を行う前に、再読み込みしてネイティブの状態を確認してください。"
        : "The operation result could not be confirmed. Refresh and inspect the native state before making another change.",
    );
    await expect(
      page.getByRole("button", { name: ja ? "追加" : "Add", exact: true }),
    ).toBeDisabled();
    await open(page, language, "&toolsState=providerError");
    await expect(page.getByRole("alert")).toHaveText(
      ja
        ? "ネイティブのツール設定を読み込めませんでした。解析エラーの詳細は表示しません。"
        : "Could not read a native tool configuration. Raw parse diagnostics are withheld.",
    );
    await expect(
      page.getByRole("button", {
        name: ja ? "定義を読み込む" : "Read definition",
        exact: true,
      }),
    ).toBeDisabled();
    await expect(page.locator("body")).not.toContainText("fixture private");
  });
}
