import type {
  DesignNote,
  DesignNoteAddition,
  DesignNoteWrite,
} from "../../../shared/types";

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
  const addition: DesignNoteAddition = {
    id: "addition-fixture",
    note_id: note.id,
    heading: "Original addition 日本語",
    content: "Original proposal 内容",
    kind: "proposal",
    speaker: "Speaker 原文",
    source: "Conversation #42 原文",
    recorder: "agent-recorder",
    referenced_body_revision: 1,
    short_quote: "Original quote 原文",
    hidden: false,
    revision: 1,
    editor: "original-editor",
    created_at: at,
    updated_at: at,
  };
  const history = [structuredClone(note)];
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
      if (state === "loading") return new Promise<Response>(() => {});
      if (state === "error") return json(null, 503);
      if (url.pathname === root) return json(state === "empty" ? [] : [note]);
      if (url.pathname.endsWith("/additions/addition-fixture/history"))
        return json([addition]);
      if (url.pathname.endsWith("/history")) return json(history);
      return json({ note, additions: [addition] });
    }
    const request = body as DesignNoteWrite;
    writes.push(request);
    if (params.get("notesSave") === "conflict") {
      note = {
        ...note,
        revision: note.revision + 1,
        title: "Remote title 原文",
        body: "Remote body 原文",
      };
      return json(null, 409);
    }
    if (params.get("notesSave") === "invalid") return json(null, 400);
    if (params.get("notesSave") === "unknown") return json(null, 503);
    const action = request.operation;
    if (action.action === "create" || action.action === "update") {
      note = {
        ...note,
        title: action.title,
        body: action.body,
        revision: note.revision + 1,
      };
      history.push(structuredClone(note));
    }
    return json({ note, addition: null });
  };
}
