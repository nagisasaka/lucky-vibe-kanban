import assert from "node:assert/strict";
import path from "node:path";
import { randomUUID } from "node:crypto";
import { expect as baseExpect } from "@playwright/test";

const expect = baseExpect.configure({ timeout: 15000 });

// Run against the caller's disposable instance, never a shared LVK server.
export async function checkMarkdownEditing({
  page,
  panel,
  api,
  notePath,
  write,
  tool,
  project,
  output,
}) {
  const requests = [];
  const record = (request) => {
    if (
      request.method() === "POST" &&
      new URL(request.url()).pathname === notePath
    )
      requests.push(request.postDataJSON());
  };
  page.on("request", record);
  await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
  const pasted =
    "## 貼り付け\n\n- 項目その一\n- 項目その二\n\n既存\\_エスケープ 日本語 😀\n\n";
  const raw = "## 手入力\n\n" + pasted;
  async function enterMarkdown(editor) {
    const textarea = editor.locator("textarea");
    await textarea.fill("");
    await textarea.pressSequentially("## ");
    await expect(textarea).toHaveValue("## ");
    assert.equal(await editor.locator("[contenteditable=true], h2").count(), 0);
    await page.keyboard.insertText("手入力\n\n");
    // Use Chromium's native paste default action, including its text/plain path.
    await page.evaluate((text) => navigator.clipboard.writeText(text), pasted);
    await textarea.press("Control+V");
    await expect(textarea).toHaveValue(raw);
    return raw;
  }
  async function preview(editor, snapshot) {
    const before = await snapshot();
    const writes = requests.length;
    const draft = await editor.locator("textarea").inputValue();
    await editor.getByRole("tab", { name: "プレビュー", exact: true }).click();
    const rendered = editor.getByRole("tabpanel");
    await expect(
      rendered.getByRole("heading", { name: "手入力", exact: true }),
    ).toBeVisible();
    await expect(
      rendered.getByRole("heading", { name: "貼り付け", exact: true }),
    ).toBeVisible();
    await expect(rendered.getByRole("listitem")).toHaveCount(2);
    await expect(editor.locator("textarea")).toBeHidden();
    // Exercise the standard keyboard interaction as well as pointer tab changes.
    await editor
      .getByRole("tab", { name: "プレビュー", exact: true })
      .press("ArrowLeft");
    await expect(
      editor.getByRole("tab", { name: "編集", exact: true }),
    ).toBeFocused();
    await expect(editor.locator("textarea")).toHaveValue(draft);
    assert.deepEqual(
      await snapshot(),
      before,
      "preview must not change stored data or revisions",
    );
    assert.equal(requests.length, writes, "preview must not send a write");
  }
  async function renderedHtml(container) {
    const root = container.locator(".wysiwyg [data-lexical-editor]");
    await expect(root).toHaveAttribute("contenteditable", "false");
    return root.innerHTML();
  }
  const bodyEditor = panel.getByTestId("note-body-editor");
  const bodySave = panel.getByRole("button", { name: "保存", exact: true });
  await panel.getByRole("button", { name: "新しい設計メモ" }).click();
  await panel.getByLabel("タイトル", { exact: true }).fill("生テキスト検証");
  await enterMarkdown(bodyEditor);
  await preview(bodyEditor, async () => (await api(notePath)).data);
  await expect(panel.getByLabel("タイトル", { exact: true })).toHaveValue(
    "生テキスト検証",
  );
  await bodySave.click();
  await expect(
    panel
      .getByTestId("note-body")
      .getByRole("heading", { name: "貼り付け", exact: true }),
  ).toBeVisible();
  const note = (await api(notePath)).data.find(
    (n) => n.title === "生テキスト検証",
  );
  assert.equal(note.body, raw);
  const detail = async () => (await api(`${notePath}/${note.id}`)).data;
  assert.equal((await detail()).note.revision, 1);

  // Existing body: raw typing/paste, metadata retention, and a committed response
  // lost on the network. Retrying must reuse the request ID and create one version.
  await panel.getByRole("button", { name: "本文を編集" }).click();
  await enterMarkdown(bodyEditor);
  await panel
    .getByLabel("タイトル", { exact: true })
    .fill("生テキスト検証・編集");
  await panel.getByLabel("変更理由（任意）").fill("表示を保存前に確認");
  await preview(bodyEditor, detail);
  await expect(panel.getByLabel("タイトル", { exact: true })).toHaveValue(
    "生テキスト検証・編集",
  );
  await expect(panel.getByLabel("変更理由（任意）")).toHaveValue(
    "表示を保存前に確認",
  );
  let lost = false;
  const loseResponse = async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    const response = await route.fetch();
    assert.equal(response.status(), 200);
    lost = true;
    await route.abort("failed");
  };
  await page.route(`**${notePath}`, loseResponse);
  await bodySave.click();
  await expect(panel.getByRole("alert")).toContainText("入力は保持");
  assert.ok(lost);
  await page.unroute(`**${notePath}`, loseResponse);
  await expect(bodyEditor.locator("textarea")).toHaveValue(raw);
  await expect(panel.getByLabel("変更理由（任意）")).toHaveValue(
    "表示を保存前に確認",
  );
  assert.equal((await detail()).note.revision, 2);
  await preview(bodyEditor, detail);
  const lostRequest = requests.at(-1);
  await bodySave.click();
  await expect(bodyEditor).toHaveCount(0);
  assert.deepEqual(requests.at(-1), lostRequest);
  assert.equal((await detail()).note.revision, 2);
  assert.equal((await detail()).note.body, raw);
  assert.equal((await detail()).note.change_reason, "表示を保存前に確認");

  // New addition and correction share the same text + preview contract.
  await panel.getByRole("button", { name: "追記する", exact: true }).click();
  const additionEditor = panel.getByTestId("addition-editor");
  const additionForm = additionEditor.locator("..");
  for (const [label, value] of [
    ["短い見出し", "生テキストの追記"],
    ["発言者", "ユーザー"],
    ["出典（会話・発言の参照）", "test/plain-markdown"],
  ])
    await additionForm.getByLabel(label, { exact: true }).fill(value);
  await additionForm.getByLabel("短い引用（任意）").fill("日本語");
  await enterMarkdown(additionEditor);
  await preview(additionEditor, detail);
  await expect(additionForm.getByLabel("出典（会話・発言の参照）")).toHaveValue(
    "test/plain-markdown",
  );
  await expect(additionForm.getByLabel("発言者", { exact: true })).toHaveValue(
    "ユーザー",
  );
  await expect(additionForm.getByRole("combobox")).toHaveValue("open_question");
  await expect(additionForm.getByLabel("参照本文revision")).toHaveValue("2");
  await expect(additionForm.getByLabel("短い引用（任意）")).toHaveValue(
    "日本語",
  );
  await additionForm.getByRole("button", { name: "追記を保存" }).click();
  await expect(additionEditor).toHaveCount(0);
  let stored = (await detail()).additions[0];
  assert.equal(stored.content, raw);
  const entry = panel.getByTestId("note-addition");
  await expect(
    entry.getByRole("heading", { name: "貼り付け", exact: true }),
  ).toBeVisible();
  await entry.getByRole("button", { name: "追記を訂正" }).click();
  await enterMarkdown(additionEditor);
  await additionForm
    .getByLabel("出典（会話・発言の参照）")
    .fill("test/correction");
  await preview(additionEditor, detail);
  await expect(additionForm.getByLabel("出典（会話・発言の参照）")).toHaveValue(
    "test/correction",
  );
  // Validation failure leaves the entire correction draft and preview available.
  await additionEditor.locator("textarea").fill("😀".repeat(100001));
  await additionForm.getByRole("button", { name: "追記を保存" }).click();
  await expect(panel.getByRole("alert")).toContainText("入力は保持");
  await expect(additionEditor.locator("textarea")).toHaveValue(
    "😀".repeat(100001),
  );
  assert.equal((await detail()).additions[0].revision, stored.revision);
  await additionEditor.locator("textarea").fill(raw);
  await preview(additionEditor, detail);
  await additionForm.getByRole("button", { name: "追記を保存" }).click();
  await expect(additionEditor).toHaveCount(0);
  stored = (await detail()).additions[0];
  assert.equal(stored.content, raw);
  assert.equal(stored.source, "test/correction");
  assert.equal(stored.revision, 2);

  // A historical rich-editor string / MCP addition is never re-serialized by an
  // unchanged edit, even when previewed. Cover escaped punctuation and whitespace.
  const legacy =
    "## 既存メモ\n\n日本語の\\_名前\\_ と \\*記号\\*\n\n\n末尾の空行も保持\n\n";
  let current = await detail();
  await write({
    action: "update",
    note_id: note.id,
    expected_revision: current.note.revision,
    title: current.note.title,
    body: legacy,
  });
  const legacyAddition = {
    heading: stored.heading,
    content: legacy,
    kind: stored.kind,
    source: stored.source,
    speaker: stored.speaker,
    referenced_body_revision: stored.referenced_body_revision,
    short_quote: null,
  };
  await tool("write_design_note", {
    project_id: project,
    input: {
      request_id: randomUUID(),
      operation: {
        action: "correct_addition",
        note_id: note.id,
        addition_id: stored.id,
        expected_revision: stored.revision,
        addition: legacyAddition,
      },
    },
  });
  await expect(panel.getByTestId("note-body")).toContainText(
    "末尾の空行も保持",
  );
  await expect(entry).toContainText("末尾の空行も保持");
  const bodyHtml = await renderedHtml(panel.getByTestId("note-body"));
  const additionHtml = await renderedHtml(entry);
  for (const kind of ["body", "addition"]) {
    await panel
      .getByRole("button", {
        name: kind === "body" ? "本文を編集" : "追記を訂正",
        exact: true,
      })
      .click();
    const editor = kind === "body" ? bodyEditor : additionEditor;
    await expect(editor.locator("textarea")).toHaveValue(legacy);
    await editor.getByRole("tab", { name: "プレビュー", exact: true }).click();
    assert.equal(
      await renderedHtml(editor),
      kind === "body" ? bodyHtml : additionHtml,
    );
    await editor.getByRole("tab", { name: "編集", exact: true }).click();
    await expect(editor.locator("textarea")).toHaveValue(legacy);
    await panel
      .getByRole("button", {
        name: kind === "body" ? "保存" : "追記を保存",
        exact: true,
      })
      .click();
    await expect(editor).toHaveCount(0);
    assert.equal(
      kind === "body"
        ? (await detail()).note.body
        : (await detail()).additions[0].content,
      legacy,
    );
  }
  assert.equal(await renderedHtml(panel.getByTestId("note-body")), bodyHtml);
  assert.equal(await renderedHtml(entry), additionHtml);

  // Body empty/whitespace is allowed. Enforce Unicode count on the server rather
  // than HTML maxlength (which counts UTF-16 units and would reject emoji early).
  await panel.getByRole("button", { name: "本文を編集" }).click();
  await bodyEditor.locator("textarea").fill("😀".repeat(100001));
  await bodySave.click();
  await expect(panel.getByRole("alert")).toContainText("入力は保持");
  await expect(bodyEditor.locator("textarea")).toHaveValue("😀".repeat(100001));
  await panel.getByRole("button", { name: "キャンセル", exact: true }).click();
  for (const empty of ["", " \n\t"]) {
    await panel.getByRole("button", { name: "本文を編集" }).click();
    await bodyEditor.locator("textarea").fill(empty);
    await bodySave.click();
    await expect(bodyEditor).toHaveCount(0);
    assert.equal((await detail()).note.body, empty);
  }
  await panel.getByRole("button", { name: "追記する", exact: true }).click();
  for (const empty of ["", " \n\t"]) {
    await additionEditor.locator("textarea").fill(empty);
    await expect(
      additionForm.getByRole("button", { name: "追記を保存" }),
    ).toBeDisabled();
  }
  await additionForm
    .getByRole("button", { name: "キャンセル", exact: true })
    .click();
  current = await detail();
  // Accepted boundary is exactly 100000 Unicode characters, including non-BMP.
  const maxText = "😀".repeat(100000);
  await write({
    action: "update",
    note_id: note.id,
    expected_revision: current.note.revision,
    title: current.note.title,
    body: maxText,
  });
  await expect(panel.getByTestId("note-body")).toContainText("😀");
  await panel.getByRole("button", { name: "本文を編集" }).click();
  await expect(bodyEditor.locator("textarea")).toHaveValue(maxText);
  await bodySave.click();
  await expect(bodyEditor).toHaveCount(0);
  assert.equal((await detail()).note.body, maxText);
  await entry.getByRole("button", { name: "追記を訂正" }).click();
  await additionEditor.locator("textarea").fill(maxText);
  await additionForm.getByRole("button", { name: "追記を保存" }).click();
  await expect(additionEditor).toHaveCount(0);
  assert.equal((await detail()).additions[0].content, maxText);

  // Narrow-screen edit/preview usability and native newline normalization.
  await page.setViewportSize({ width: 320, height: 844 });
  await page.goto(`${new URL(page.url()).origin}/projects/${project}`);
  await page.getByRole("button", { name: "設計メモ", exact: true }).click();
  await panel.getByRole("button", { name: /生テキスト検証・編集/ }).click();
  await panel.getByRole("button", { name: "本文を編集" }).click();
  await bodyEditor.locator("textarea").fill("## 手入力\r\n\r\n" + pasted);
  assert.equal(
    (await bodyEditor.locator("textarea").inputValue()).replace(/\r\n/g, "\n"),
    raw,
  );
  await preview(bodyEditor, detail);
  assert.ok(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  );
  await page.screenshot({
    path: path.join(output, "screenshots/plain-edit-mobile.png"),
  });
  await bodyEditor
    .getByRole("tab", { name: "プレビュー", exact: true })
    .click();
  assert.ok(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  );
  await page.screenshot({
    path: path.join(output, "screenshots/plain-preview-mobile.png"),
  });
  // Saving from preview remains explicit; CRLF preservation or normalization is allowed.
  await bodySave.click();
  await expect(bodyEditor).toHaveCount(0);
  assert.equal((await detail()).note.body.replace(/\r\n/g, "\n"), raw);
  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.goto(`${new URL(page.url()).origin}/projects/${project}`);
  await page.getByRole("button", { name: "設計メモ", exact: true }).click();
  page.off("request", record);
}
