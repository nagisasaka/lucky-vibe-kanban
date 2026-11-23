# Saved filters

A saved filter is a Project-owned condition, identified by a stable UUID. It is
shared by the UI and both MCP transports. Changing its name or conditions retains
its ID. Deleting it deletes its derived cache, never Issues or relationships.

## HTTP

All paths below start with `/api/projects/{project_id}/saved-filters`. Responses
use the existing `{success, data, message}` envelope. The authenticated principal
needs `lvk:read`; changes and evaluation starts also require `lvk:write`.

| Method | Suffix | Input | `data` |
| --- | --- | --- | --- |
| GET | (none) | query `offset=0&limit=100` | `ListSavedFiltersResponse` |
| POST | (none) | `{request_id, definition}` | `SavedFilter` |
| GET | `/{id}` | none | `SavedFilter` |
| PUT | `/{id}` | `{request_id, expected_revision, definition}` | `SavedFilter` |
| DELETE | `/{id}` | `{request_id, expected_revision}` | `{deleted, saved_filter_id, revision}` |
| POST | `/{id}/evaluate` | `{offset:0, limit:100, retry_failed:false}` | `SavedFilterEvaluation` |
| GET | `/{id}/results` | query `offset=0&limit=100` | `SavedFilterEvaluation` |
| GET | `/{id}/link` | none | `SavedFilterLink` |

The definition is `{name, machine, natural_language, context}`. Text fields
`natural_language` and `context` can be null. Updating replaces the full
definition. Reuse the same request UUID and input after a lost CRUD response:
the original committed result is returned even after later edits or deletion.
Changing the input for an existing request UUID or editing an old revision
returns 409. A missing definition, including a different Project's ID, returns
404. Invalid conditions return 400. UI types are generated into
`shared/remote-types.ts` from `api-types`.

List and result pages default to 100 and cap at 500. Follow `next_offset` until
null. `total_count` is the unpaged count. Result responses include an opaque
`evaluation_revision` fingerprint of all ordered IDs, evaluation state, stale/error
and definition revisions. For every subsequent result page pass it as
`expected_evaluation_revision` (query/body for HTTP, `page` for MCP). Missing tokens
at offset > 0 return 400. A mismatch returns HTTP 409
(message prefix `saved_filter_evaluation_changed`); MCP reports a conflict. Discard collected pages
and restart at offset 0 without the token. This also detects same-count membership
swaps and relative-time membership changes. Definition lists do not use this token. Evaluation
always receives the entire Project corpus, including archived and noncandidate
Issues; it never evaluates only the returned page.

## Machine conditions

Conditions in different fields combine with AND. Empty lists impose no
restriction. Nonempty `status_ids`, `priorities` and `assignee_ids` use OR.
`include_unassigned` defaults to false for existing definitions. When true, the
assignee field matches selected user IDs OR Issues with no assignees. With no
selected IDs it matches only unassigned Issues. Other fields still combine with AND.
`tag_ids` uses `tag_mode: "any" | "all"` (default `any`). Priority values are
`urgent`, `high`, `medium`, `low`.

`search` preserves existing MCP semantics: case-insensitive literal substring of
full title or description, including literal whitespace. It does not search the
simple ID or expand relationships. `simple_id` separately provides a
case-insensitive exact match, consistent with existing MCP IssueFilters. Null
imposes no restriction; whitespace and bare numbers are not expanded. Existing
ordinary UI search (title/simple ID/number/related) is unchanged.

`archived` is `active` (default), `archived`, or `all`, and refers only to Issue
archive. It does not imply hidden-status or Workspace archive filtering.

`created_at` and `updated_at` independently accept either:

- `{kind:"absolute", start:<RFC3339|null>, end:<RFC3339|null>}`: inclusive start,
  exclusive end. At least one bound is required. Both bounds require start < end.
- `{kind:"relative", seconds:<positive integer>}`: inclusive `[now-seconds, now]`,
  with a single server UTC evaluation time. No implicit local calendar or timezone.

`relation: {kind, target_issue_id}` selects Issues relative to a same-Project
target, excluding the target itself:

| Kind | Selected Issues |
| --- | --- |
| `parent` | The target's direct parent |
| `children` | Direct children of the target |
| `descendants` | All descendants along child links, with cycle protection |
| `related` | Either endpoint of an explicit `related` edge incident to target, one hop |
| `blocking` | Sources of explicit blocking edges pointing to target, one hop |
| `blocked_by` | Targets of explicit blocking edges from target, one hop |

Other conditions apply to the resulting Issues. Traversal can pass through
archived or otherwise excluded intermediate Issues. Duplicate edges are not
related edges; blocking cycles do not imply transitive traversal.

## Evaluation and links

`SavedFilterEvaluation` contains `saved_filter_id`, `project_id`, `state`,
`issue_ids`, paging metadata, `stale`, `error`, `definition_revision`, and
`evaluation_revision`.
`state` is `unevaluated`, `evaluating`, `complete`, or `failed`. An empty ID list
in an incomplete/failed evaluation is not a successful zero-match answer.
Previous results may be returned with `stale: true`. No natural-language condition
means immediate machine-only evaluation without AI. `results` reads cache and
current machine conditions without starting AI; `evaluate` may start semantic
work and reuses valid semantic results. Failed identical input is retried only
with explicit `retry_failed:true`, preventing polling from repeatedly starting failed runs. Apply-time and active-view refreshes use
these same services; unused filters do not require a background evaluator.

A link references the stable ID: `/projects/{project_id}?savedFilter={id}`.
It applies the current definition, including after renaming. HTTP returns a
same-origin path; MCP prefixes the configured public base URL when available.
Deleted or foreign definitions do not yield a link. No remote screen control is
performed.

The common adapter maps the definition and full corpus to `SemanticRequest`,
passes machine candidate IDs separately, and invokes the semantic module with the
existing Deployment, `activate` and `retry_failed`. The corpus includes all Project Issues,
statuses, tag definitions, Issue tags, assignees and explicit relationships, read
in one SQLite snapshot. Semantic context also contains `assignee_display_names`:
a stable user-ID-ordered array of `{user_id, display_name}` for active
`access_members` actually assigned to this Project's Issues. Duplicate assignments
produce one entry. Unassigned members, other Projects, inactive/missing member
profiles, email, private profile fields and viewer-dependent SELF aliases are not
included. The existing visible-member display name is used. Renaming an assigned
member changes semantic context and invalidates its cached evidence; identical
names and evidence preserve reuse. Definition context is saved, not taken from a hidden chat.
The module owns opaque `state_json` plus CAS `generation` in
`saved_filter_semantic_cache(filter_id, generation, state_json)` and must
invalidate on relevant evidence changes, including noncandidate Issues for global
comparisons. Machine candidate changes due solely to time can reuse semantic work.

## MCP

Both transports expose `list_saved_filters`, `get_saved_filter`,
`create_saved_filter`, `update_saved_filter`, `delete_saved_filter`,
`evaluate_saved_filter`, `get_saved_filter_results`, and `get_saved_filter_link`.
They use the same DTOs and server service. Reads require Project membership in the
connection allowlist and read scope. Writes/evaluation require write scope.

List arguments are `{project_id, page?:{offset,limit,expected_evaluation_revision}}`. Get/results/link arguments
are `{project_id,saved_filter_id,page?:{offset,limit,expected_evaluation_revision}}`. Create arguments are
`{project_id,input:<HTTP create body>}`; update/delete also include
`saved_filter_id`. Evaluate arguments are
`{project_id,saved_filter_id,request_id,retry_failed?:false,page?:{offset,limit,expected_evaluation_revision}}`.

MCP mutations/evaluation starts return existing durable Operations receipts.
Acceptance is not completion. Use `get_operation` with the original `request_id`
and Project to recover the outcome; reuse identical input when retrying. Result
retrieval and HTTP evaluate do not claim a completed semantic result until the
semantic module reports completion.
