# Check results when asked

Use `list_workspace_runs` to select a Run, then read its `get_execution` result;
choose `input`, `goal` or `final_response` and follow its revision pages. A finished turn is not proof that
all requirements or tests passed.

For an independent re-review, use `query_repository` with explicit reviewed/new
full SHAs and `comparison:"direct"` to exclude current dirty changes. Use
`comparison:"merge_base"` only when intended; merge commits require explicit
parent selection when no base is provided. Follow cursors and check exclusions.
`list_workspace_diff` and `get_workspace_diff` expose current per-path changes;
`list_workspace_files` and `get_workspace_file` expose tracked reference code.
Binary, oversized or unreadable content is not an empty diff. Renames are shown
as separate source/destination paths. `list_workspace_wiki` and
`get_workspace_wiki_page` read the selected checkout; they do not prove that the
canonical Wiki has been published. Fetch only the relevant paths/pages.

Use `list_project_merge_status` to locate outstanding or unknown Workspace/repository
changes and `get_workspace_merge_status` for a selected pair. Counts refer to
local target refs without fetching. `included` records target-history evidence
for the observed source with no observed worktree changes; it is not proof of
tests, remote publication or Wiki completion. Unknown is not a clean result.

Use `list_integrations` and `get_integration` for Auto-merge results. Read the
`summary`, `sources` or `validation` section as needed. Separate execution
status, verification outcomes/exceptions, target publication and current Wiki
receipts. Missing, skipped, incomplete or unknown checks are not passes. A
partial publication is not whole-operation success. Report unresolved states and
the returned LVK link without filling gaps from the conversation's memory.
