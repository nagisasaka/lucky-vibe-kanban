import assert from "node:assert/strict";
import { expect } from "@playwright/test";
import { randomUUID } from "node:crypto";
import path from "node:path";

// Called by isolated.mjs against its fresh real backend; no route mocks.
export async function checkCategoryLayout({
  page,
  grid,
  api,
  todoPath,
  output,
}) {
  const fixtures = [];
  for (let i = 0; i < 24; i++) {
    fixtures.push(
      (
        await api(todoPath, "POST", {
          id: randomUUID(),
          category: `00-layout-${String(i).padStart(2, "0")}`,
          body:
            i === 0
              ? "Tall category\n".repeat(30)
              : i % 4 === 0
                ? `Long unbroken text ${"x".repeat(180)}`
                : `Short category ${i}`,
        })
      ).data,
    );
  }
  const layout = grid.getByTestId("todo-categories");
  await expect(grid.locator(`[data-todo-id="${fixtures[23].id}"]`)).toHaveCount(
    1,
    { timeout: 15000 },
  );

  let expectedCategories = fixtures.map((f) => f.category);
  async function geometry(label) {
    await expect
      .poll(
        async () =>
          layout.evaluate((container) => {
            const cards = [...container.children].map((cell) =>
              cell.querySelector("section").getBoundingClientRect(),
            );
            const bounds = container.getBoundingClientRect();
            return cards.every(
              (a, i) =>
                a.left >= bounds.left - 1 &&
                a.right <= bounds.right + 1 &&
                a.bottom <= bounds.bottom + 1 &&
                cards.every(
                  (b, j) =>
                    i === j ||
                    a.right <= b.left ||
                    b.right <= a.left ||
                    a.bottom <= b.top ||
                    b.bottom <= a.top,
                ),
            );
          }),
        { message: `${label}: categories must fit and never overlap` },
      )
      .toBe(true);
    const info = await layout.evaluate((container) => {
      const cards = [...container.children].map((cell) => {
        const section = cell.querySelector("section");
        const rect = section.getBoundingClientRect();
        return {
          x: rect.x,
          y: rect.y,
          bottom: rect.bottom,
          name: section.querySelector("h3").childNodes[0].textContent.trim(),
        };
      });
      return {
        cards,
        columns:
          getComputedStyle(container).gridTemplateColumns.split(" ").length,
      };
    });
    for (let i = 1; i < info.cards.length; i++) {
      assert.ok(
        info.cards[i].y >= info.cards[i - 1].y - 1,
        `${label}: visual category order`,
      );
    }
    assert.deepEqual(
      info.cards.slice(0, expectedCategories.length).map((c) => c.name),
      expectedCategories,
    );
    assert.ok(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= innerWidth,
      ),
      `${label}: page overflow`,
    );
    console.log(
      `PASS layout ${label}: ${info.cards.length} categories, ${info.columns} columns, no overlaps/overflow, ordered`,
    );
    return info;
  }

  let info = await geometry("wide mixed heights");
  assert.ok(info.columns > 1);
  assert.ok(
    info.cards[info.columns].y < info.cards[0].bottom - 100,
    "a short column must receive the next category before the tall first category ends",
  );

  console.log(
    `PASS compaction: next category starts ${Math.round(info.cards[0].bottom - info.cards[info.columns].y)}px before the tall category ends`,
  );
  const serverItems = (
    await api(`${todoPath}?include_completed=true&limit=200`)
  ).data.items;
  const domIds = await layout
    .locator("section")
    .evaluateAll((sections) =>
      sections.map((section) =>
        [...section.querySelectorAll("[data-todo-id]")].map(
          (row) => row.dataset.todoId,
        ),
      ),
    );
  for (const ids of domIds) {
    const category = serverItems.find((item) => item.id === ids[0]).category;
    assert.deepEqual(
      ids,
      serverItems
        .filter((item) => item.category === category)
        .map((item) => item.id),
      "item order matches API",
    );
  }
  await layout.evaluate((el) => {
    el.parentElement.scrollTop = 0;
  });
  await page.screenshot({
    path: path.join(output, "screenshots/layout-mixed.png"),
  });

  const row = grid.locator(`[data-todo-id="${fixtures[1].id}"]`);
  await row
    .getByRole("button", { name: fixtures[1].body, exact: true })
    .click();
  const editor = row.getByLabel("TODOを編集", { exact: true });
  await editor.fill("Expanded editor\n".repeat(20));
  await editor.evaluate((el) => {
    el.style.height = "600px";
  });
  await geometry("expanded/resized edit form");
  await row.getByRole("button", { name: "保存", exact: true }).click();
  await expect(editor).toHaveCount(0);
  await geometry("saved multiline body");

  // Tab follows the unchanged category DOM order.
  await row.getByRole("button", { name: "TODOの操作", exact: true }).focus();
  await page.keyboard.press("Tab");
  await expect(
    grid.locator(`[data-todo-id="${fixtures[2].id}"]`).getByRole("checkbox"),
  ).toBeFocused();

  for (const width of [1000, 390, 320, 1600]) {
    await page.setViewportSize({ width, height: 900 });
    info = await geometry(`viewport ${width}`);
    if (width <= 390) assert.equal(info.columns, 1);
    const last = grid.locator(`[data-todo-id="${fixtures[23].id}"]`);
    await last
      .getByRole("button", { name: fixtures[23].body, exact: true })
      .click();
    await expect(last.getByLabel("TODOを編集", { exact: true })).toBeVisible();
    await last.getByRole("button", { name: "キャンセル", exact: true }).click();
    await geometry(`viewport ${width} after edit/cancel`);
    await page.screenshot({
      path: path.join(output, `screenshots/layout-${width}.png`),
    });
  }

  const completedRow = grid.locator(`[data-todo-id="${fixtures[22].id}"]`);
  await grid.getByLabel("完了済みも表示", { exact: true }).uncheck();
  await completedRow.getByRole("checkbox").click();
  await expect(completedRow).toHaveCount(0);
  expectedCategories = fixtures
    .filter((_, i) => i !== 22)
    .map((f) => f.category);
  await geometry("completion hides a category");
  await grid.getByLabel("完了済みも表示", { exact: true }).check();
  await expect(completedRow.getByRole("checkbox")).toBeChecked();
  expectedCategories = fixtures.map((f) => f.category);
  await geometry("completed category shown again");
  await completedRow.getByRole("checkbox").click();
  await expect(completedRow.getByRole("checkbox")).not.toBeChecked();
  fixtures[22] = (await api(`${todoPath}/${fixtures[22].id}`)).data;

  await row.getByRole("button", { name: "TODOの操作", exact: true }).click();
  await page.getByRole("menuitem", { name: "削除", exact: true }).click();
  await geometry("delete confirmation expanded");
  await row.getByRole("button", { name: "削除", exact: true }).click();
  await expect(row).toHaveCount(0);
  assert.equal(
    (await api(`${todoPath}/${fixtures[1].id}`, "GET", undefined, 404)).success,
    false,
  );
  expectedCategories = expectedCategories.filter(
    (name) => name !== fixtures[1].category,
  );
  await geometry("category removed");
  // Remove fixtures through the real API; the existing suite then continues.
  for (const fixture of fixtures.filter((_, i) => i !== 1)) {
    await api(
      `${todoPath}/${fixture.id}?expected_revision=${fixture.revision}`,
      "DELETE",
    );
  }
  await expect(grid.locator(`[data-todo-id="${fixtures[0].id}"]`)).toHaveCount(
    0,
    { timeout: 15000 },
  );
  await page.setViewportSize({ width: 1600, height: 1000 });
}
