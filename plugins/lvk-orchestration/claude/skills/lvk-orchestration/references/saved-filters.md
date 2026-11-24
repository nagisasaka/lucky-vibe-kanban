# Saved filters

## Scope and availability

A saved filter is a Project-shared **condition**, not a saved fixed Issue set or a
private viewer preference. UI, stdio MCP and remote MCP share its definition and
evaluation. Its stable ID survives renaming and condition edits. Deleting a filter
removes its definition/cache, never Issues, tags or relationships.

Use these operations only within the user's requested scope. A question about
filtering does not authorize creating a persistent definition or starting AI.
For an ordinary one-off search, use existing search tools. For an existing filter,
read its definition/results or return its link without an unnecessary evaluation.
Do not turn a filtering request into Issue or relationship edits.

First check the **connected** MCP catalog and schemas. Source/Skill support does
not prove deployment, exposure or permission; remote and stdio connections may
run different versions. If a required tool is absent, report what remains
unavailable or unsaved rather than claim success or silently use another endpoint.
All calls require an accessible Project and `lvk:read`; create/update/delete and
evaluation also require `lvk:write`. Respect the remote Project allowlist and
stdio's authenticated internal bridge; do not bypass either boundary.

## Tools and arguments

These are the shared logical tool names; use the connected client's actual names.
All calls require `project_id` (UUID); `saved_filter_id` is also a UUID.

| Tool | Additional arguments | Purpose / response |
| --- | --- | --- |
| `list_saved_filters` | optional `page` | List shared definitions; follow `next_offset`. |
| `get_saved_filter` | `saved_filter_id` | Read the current definition and `revision`. |
| `create_saved_filter` | `input: {request_id, definition}` | Create conditions; operation receipt. |
| `update_saved_filter` | `saved_filter_id`, `input: {request_id, expected_revision, definition}` | Replace the full definition at its current revision; receipt. |
| `delete_saved_filter` | `saved_filter_id`, `input: {request_id, expected_revision}` | Delete only the definition/cache; receipt. |
| `evaluate_saved_filter` | `saved_filter_id`, **top-level** `request_id`; optional `retry_failed` (default false), `page` | Evaluate current conditions, reuse valid cache; may start server-side AI; receipt. |
| `get_saved_filter_results` | `saved_filter_id`, optional `page` | Current machine results and semantic cache/state; never starts AI. |
| `get_saved_filter_link` | `saved_filter_id` | Return the stable board `url` for the current definition. |

`get_saved_filter` and `get_saved_filter_link` accept the same optional `page`
shape as result reads but do not paginate their single-object response.
`definition` is `{name, machine, natural_language, context}`: `name` is required,
`machine` defaults to `{}`, and the two text fields may be null. `context` is a
saved string explaining referents such as "this release", not hidden chat history.
Use machine-only, natural-language-only, or combined conditions as requested;
combined results must satisfy both. Read before editing, preserve fields outside
the requested change, and use the returned `revision` as `expected_revision`.

Machine fields combine with AND. Nonempty `status_ids`, `priorities` and
`assignee_ids` selections use OR; empty lists impose no restriction. Priorities
are `urgent`, `high`, `medium`, `low`. `include_unassigned:true` adds unassigned
Issues to selected assignees (or selects only unassigned when no IDs are supplied).
`tag_ids` uses `tag_mode:"any"` (default) or `"all"`. Use existing Project IDs/tags.

- `search`: case-insensitive literal title/body substring, including whitespace.
  `simple_id`: separate case-insensitive exact match. Ordinary UI search and its
  related-Issue expansion retain their existing meanings.
- `archived`: `active` (default), `archived` or `all`, referring to Issue archive,
  not status visibility or Workspace archive.
- `created_at` / `updated_at`: `{kind:"absolute", start, end}` uses RFC3339 bounds,
  inclusive start/exclusive end, at least one non-null bound and start < end when
  both exist; `{kind:"relative", seconds:<positive integer>}` uses `[now-seconds,now]`.
- `relation:{kind,target_issue_id}`: a same-Project target, excluded from results.
  `parent`/`children` select its direct parent/children; `descendants` traverses child
  links; `related` selects either endpoint of explicit related edges, one hop.
  `blocking` selects Issues blocking the target; `blocked_by` selects Issues the
  target blocks, one hop. These are selectors, not relationship mutations or new
  relationship semantics. Other machine conditions still apply to selected Issues.

## Use and recover

1. Resolve the Project and existing definitions before creating duplicates. When
   asked to save or edit conditions, perform only that authorized change, resolve
   its receipt, then read back the definition/ID/revision. CRUD does not itself
   mean semantic evaluation completed. A revision conflict requires rereading
   and reconciling the intended edit, not overwriting another writer's change.
2. For authorized evaluation, inspect current results first and reuse a valid
   result. Call `evaluate_saved_filter` only when a current evaluation is needed.
   Machine-only conditions need no AI; natural-language evaluation uses LVK's
   existing server-side execution and the full Project corpus, not just one page.
3. Resolve the evaluation receipt and inspect `get_saved_filter_results` for
   progress. Receipt `succeeded` means the operation returned successfully, not
   necessarily that semantic state is `complete`. Results reads never launch AI;
   staged evaluations can require further authorized `evaluate_saved_filter`
   calls to advance. Use bounded, paced follow-ups while needed, not a background
   evaluator for unused filters. Keep normal `retry_failed:false`; explicit retry
   of failed identical input uses `retry_failed:true` only within retry authorization.
4. Return the `url` from `get_saved_filter_link`. It selects
   `/projects/{project_id}?savedFilter={id}` and uses current conditions after
   rename/edit; opening it may trigger needed evaluation in the UI. MCP prefixes
   the configured public base URL when available; do not invent a host if it is
   relative. A link does not remotely switch an already-open screen. A deleted,
   invalid or foreign filter is an error, not an empty result.

For mutations/evaluation, `request_id` is a stable UUID. Recover uncertain writes
with `get_operation` using the same Project/request ID; retry the **identical full
input with the same ID**. Do not rotate IDs after a timeout or reuse one with a
different payload. A new intentional edit/evaluation/continuation uses a new ID;
replaying an old receipt is not a fresh evaluation. Verify operation outcome and
read back state before claiming persistence or completion.

## Results, freshness and paging

Read `state`, `stale`, `error`, `definition_revision` and `evaluation_revision`
together with `issue_ids` / `total_count`:

- `unevaluated` or `evaluating`: no final answer yet, even if IDs/count are empty.
- `failed`: report the failure; an empty list is not a successful zero-match answer.
- `stale:true`: any returned prior result is old, not current evidence.
- Only `complete`, non-stale results for the current definition can establish a
  definitive zero (`total_count:0`). An empty later page alone cannot do so.

Evidence/condition/context changes, including exclusions, time boundaries and
cross-Issue comparisons, may invalidate cache. Let the shared evaluator decide
what must be refreshed; do not treat unchanged filter revision alone as proof of
freshness or repeatedly force full AI work on unchanged inputs.

`page` is `{offset, limit, expected_evaluation_revision}` with default offset 0,
default limit 100 and maximum 500. Follow `next_offset` until null for lists and
results. For **result** pages after offset 0, pass the first page's opaque
`evaluation_revision` as `page.expected_evaluation_revision` on every later page.
It identifies the complete ordered result/state, not the definition revision.
Missing tokens are rejected; a changed evaluation produces a conflict
(`restart_result_pagination`, HTTP 409). Discard collected pages and restart at
offset 0 without the token, with bounded retries if results keep changing. Never
combine different evaluation versions or call a partial page the full set.
Definition lists do not use this token. Prefer result reads for remaining pages
instead of launching an evaluation for each page.

Repository contract sources: `docs/api/saved-filters.md`,
`crates/api-types/src/saved_filter{,_mcp}.rs`,
`crates/server/src/remote_mcp/saved_filters.rs`, and
`crates/mcp/src/task_server/tools/saved_filters.rs`. The connected schema remains
the availability check; repository paths are source references, not bundled Skill
dependencies.
