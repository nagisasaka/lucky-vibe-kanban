# Check results when asked

Read the selected run's `get_execution` result; choose `input`, `goal` or
`final_response` and follow its revision pages. A finished turn is not proof that
all requirements or tests passed.

Use `get_workspace_memory` for the selected repository and Workspace's recorded
context. Treat it as agent-authored evidence, not an independent success receipt.
`list_workspace_diff` and `get_workspace_diff` expose current per-path changes;
`list_workspace_files` and `get_workspace_file` expose tracked reference code.
Binary, oversized or unreadable content is not an empty diff. Renames are shown
as separate source/destination paths. `list_workspace_wiki` and
`get_workspace_wiki_page` read the selected checkout; they do not prove that the
canonical Wiki has been published. Fetch only the relevant paths/pages.

Use `list_integrations` and `get_integration` for formal results. Read the
`summary`, `sources` or `validation` section as needed. Separate execution
status, verification outcomes/exceptions, target publication and current Wiki
receipts. Missing, skipped, incomplete or unknown checks are not passes. A
partial publication is not whole-operation success. Report unresolved states and
the returned LVK link without filling gaps from the conversation's memory.
