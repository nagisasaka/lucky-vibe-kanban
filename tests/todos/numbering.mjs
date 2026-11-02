import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";

// All data belongs to the calling isolated instance, never a running LVK server.
export async function checkStdioNumbering({ api, tool, rpc }) {
  const projects = [];
  const originals = [];
  for (const key of ["NUMA", "NUMB"]) {
    const project = (
      await api("/api/local/v1/projects", "POST", {
        key,
        organization_id: "00000000-0000-0000-0000-000000000002",
        name: `番号独立性 ${key}`,
        color: "#6366f1",
      })
    ).data.id;
    projects.push(project);
    const todo = await tool("create_todo", {
      project_id: project,
      input: { id: randomUUID(), body: key, category: "numbering" },
    });
    assert.equal(todo.number, 1);
    assert.equal(todo.display_id, "TODO-1");
    originals.push(todo);
  }
  assert.notEqual(originals[0].id, originals[1].id);
  const read = (index, ref = "TODO-1") =>
    tool("get_todo", { project_id: projects[index], todo_id: ref });
  const state = [...originals];
  const checkBoth = async () => {
    for (const i of [0, 1]) {
      assert.deepEqual(await read(i), state[i]);
      assert.deepEqual(await read(i, originals[i].id), state[i]);
      for (const field of ["id", "number", "display_id"])
        assert.equal(state[i][field], originals[i][field]);
    }
  };
  const update = async (index, changes, ref = "TODO-1") => {
    const previous = state[index];
    state[index] = await tool("update_todo", {
      project_id: projects[index],
      todo_id: ref,
      changes: { expected_revision: previous.revision, ...changes },
    });
    assert.equal(state[index].revision, previous.revision + 1);
    await checkBoth(); // Includes every field of the untouched Project's TODO.
  };
  await checkBoth();
  // Both Projects must successfully update their own TODO-1.
  for (const i of [0, 1]) {
    await update(i, { body: `updated ${i}` });
    assert.equal(state[i].body, `updated ${i}`);
  }
  const statuses = (
    await api(
      `/api/local/v1/fallback/project_statuses?project_id=${projects[0]}`,
    )
  ).project_statuses;
  const issue = (
    await api("/api/local/v1/issues", "POST", {
      project_id: projects[0],
      status_id: statuses[0].id,
      title: "番号不変性",
      sort_order: 0,
      extension_metadata: {},
    })
  ).data.id;
  await update(0, { issue_id: issue });
  assert.equal(state[0].issue_id, issue);
  await update(0, { clear_issue: true });
  assert.equal(state[0].issue_id, null);
  // UUID mutation compatibility and stale revisions remain covered explicitly.
  await update(1, { completed: true }, originals[1].id);
  assert.equal(state[1].completed, true);
  for (const i of [0, 1]) {
    for (const ref of ["TODO-1", originals[i].id]) {
      const result = await rpc("tools/call", {
        name: "update_todo",
        arguments: {
          project_id: projects[i],
          todo_id: ref,
          changes: { expected_revision: originals[i].revision, body: "stale" },
        },
      });
      assert.equal(result.isError, true);
      const failure =
        result.structuredContent ??
        JSON.parse(result.content.find((item) => item.type === "text").text);
      assert.equal(failure.success, false);
      assert.equal(failure.diagnostic.category, "conflict");
      assert.equal(failure.diagnostic.outcome, "unknown");
      assert.equal(failure.diagnostic.recovery, "inspect_receipt_and_state");
      await checkBoth();
    }
  }
  console.log(
    "PASS numbering: two Project-local TODO-1s, isolated reads/updates, link/unlink identity, UUID compatibility and stale revisions",
  );
}
