---
name: lvk-orchestration
description: Use when discussing Lucky Vibe Kanban (LVK), including its product philosophy, UX, architecture, specifications, implementation, code investigation, or development progress. Collaborate on design in depth and use LVK MCP to delegate coding work to agents when appropriate.
---

# LVK: design collaboration and execution orchestration

Help humans stay immersed in one evolving project: pass ideas to LVK, experience implementations, and return to the next thought. Multiple AI implementations can run in parallel; parallelism serves human thinking. The coordinating AI is also a substantial specification and design partner. LVK owns durable work and scheduling; do not recreate them.

## Always apply

- The coordinating AI delegates builds and managed Workspace changes to the responsible coding agent. An already assigned coding agent implements and validates its authorised task in its own Workspace; it does not delegate the assignment back to itself.
- Respect current user intent, design decisions and authorization. Discussion alone does not authorize execution. A later explicit instruction can supersede a temporary restriction; do not reconfirm the same authorized work.
- For additional implementation on the same objective (fixes, simplification, refactoring or verification), prefer the appropriate existing Issue, Workspace **and implementation Session**. Run/Goal completion alone does not justify a new Session. Choose code isolation and conversation continuity separately; independent review can warrant fresh context.
- Write coding-agent requests and user-facing reports in the user's conversation language, unless explicitly directed otherwise. Ask coding agents, including Codex, to provide their own user-facing explanations, completion reports, questions and human-approval requests in that same language whenever the execution protocol permits. Preserve technical literals, original quotations and fixed schemas; do not translate source code, comments or repository artifacts unless the task requires it.
- In Issue specifications, reviews and investigation reports, put the conclusion first, immediately followed by any choices requiring human input. Use a few explicit `> [!DECISION]` quotes for pending human choices (an LVK extension, not a GitHub standard alert); omit empty decision sections and keep supporting evidence below. Express confirmed human intent as an ordinary labeled quote, not a dedicated alert or inferred intention. Do not mark settled constraints or informational categories as pending decisions or warnings.
- Distinguish accepted execution from completion. Normally return to the conversation after confirmed delegation; wait when the user explicitly asks for completed results. Never claim unattended monitoring without a real mechanism.
- Verify applicable merge, publication, Wiki, archive and physical deletion results separately. Unknown or failed outcomes are not success. Respect execution denials, permissions and supported recovery; retain stable request IDs after uncertain writes.

## Read when needed

Read only the references relevant to the user's intent and planned operation; each is bundled with this Skill.

| Intent or next operation | Reference |
| --- | --- |
| Discuss philosophy, UX or design; choose how much to delegate; report findings | [Using LVK](references/using-lvk.md) |
| Create/reuse Issue, Workspace or Session; start/continue execution; check status or wait | [Execution](references/execution.md) |
| Investigate source, request REVIEW, review specifications or share review evidence | [Investigation](references/investigation.md) |
| Start/check Auto-merge, validation, publication or Wiki; resume failed work; replace/transfer an Issue, close old work; archive an Issue/Workspace or clean up folders (even without a merge or transfer) | [Auto-merge, transfer and cleanup](references/auto-merge.md) |
| Record Todos, Design Notes, priority, tags or Issue relationships | [Records](references/records.md) |
| Try unmerged changes, prepare or check a preview | [Previews](references/previews.md) |
| Develop LVK itself; switch/restart its development server | [LVK development](references/lvk-development.md) |

Detailed permissions and completion gates live with their operation. Before changing an old Issue after a transfer, follow the cleanup gate in the Auto-merge reference; status changes alone do not finish cleanup.
