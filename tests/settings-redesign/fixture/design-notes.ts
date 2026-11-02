import type { DesignNote, DesignNoteWrite } from "../../../shared/types";

// Browser fixture only: no persistent state, backend or model execution.
export function createDesignNoteFixture(params: URLSearchParams) {
  const at = "2026-01-02T15:04:05Z";
  let note: DesignNote = {
    id: "note-fixture",
    project_id: "project-fixture",
    title: "原文 Title α",
    body: "Original **human body** 日本語",
    revision: 1,
    editor: "human-editor",
    change_reason: "Original reason",
    created_at: at,
    updated_at: at,
  };
  const writes: DesignNoteWrite[] = [];
  Object.defineProperty(window, "__designNoteFixture", { value: { writes } });
  const json = (data: unknown, status = 200) =>
    new Response(
      JSON.stringify({
        success: status < 400,
        data: status < 400 ? data : null,
        message: status < 400 ? null : "fixture private server detail",
      }),
      { status, headers: { "Content-Type": "application/json" } },
    );
  return (
    url: URL,
    method: string,
    body: unknown,
  ): Response | Promise<Response> | undefined => {
    const root = "/api/projects/project-fixture/design-notes";
    if (!url.pathname.startsWith(root)) return;
    const state = params.get("notesState");
    if (method === "GET") {
      if (state === "error") return json(null, 503);
      if (url.pathname === root) return json([note]);
      return json({ note, additions: [] });
    }
    const request = body as DesignNoteWrite;
    writes.push(request);
    // The retained write case exercises conflict recovery, not normal CRUD.
    note = {
      ...note,
      revision: note.revision + 1,
      title: "Remote title 原文",
      body: "Remote body 原文",
    };
    return json(null, 409);
  };
}
