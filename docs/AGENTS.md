# Documentation Guidelines

## Scope and Sources

- `docs/` contains Mintlify site pages (`.mdx`) as well as Markdown development notes, designs and implementation records. Apply site-specific rules only to site pages; preserve the format and language of other documents.
- Check the relevant source, configuration and tests before describing behaviour. Distinguish current behaviour, proposed designs and historical validation results.
- Follow the root validation guidance for documentation-only work. Ask for clarification when an unresolved ambiguity materially affects the result; resolve routine wording and structure choices from repository context.

## Writing and Structure

- Lead with the reader's goal, use clear active sentences, and use second person for procedures. Follow the surrounding document's terminology and language; use British English for new English prose.
- Explain prerequisites, steps and expected outcomes for procedures. Keep headings descriptive, links meaningful and examples focused on the task.
- Update existing documentation before adding a duplicate explanation. Link to the authoritative procedure for operational details.
- Preserve historical context in implementation records; do not present an old test result or a proposed feature as proof of current behaviour.

## Mintlify Site Pages

- Start site MDX pages with YAML frontmatter containing `title` and `description`. Follow nearby pages for optional fields and heading levels; the page title supplies the top-level heading.
- Use `docs.json` for site navigation, theme and settings. When adding, moving or removing a navigable page, update its navigation entry and internal links.
- Use site paths for internal site links, following neighbouring pages. Markdown development/design documents should use links appropriate to their existing format.
- Reuse component patterns from nearby MDX pages: `Steps` for procedures, `Tabs` for alternatives, `CodeGroup` for related examples, and callouts for information that needs emphasis. Prefer plain prose when a component adds no value.
- Use existing API documentation patterns for parameters and request/response examples. Avoid adding a generic component catalogue to a task-specific page.
- Give images descriptive alternative text and embedded media meaningful titles. Maintain readable heading order and accessible examples.

## Examples and Validation

- Label code fences with the language. Make examples complete for their stated purpose and identify values readers must replace; never include real credentials.
- Check changed commands and configuration against their actual entrypoints. Run relevant safe examples when needed to establish correctness; identify examples that could not be exercised instead of claiming they were tested.
- Validate changed links, frontmatter, navigation entries and MDX component syntax as applicable. Use existing documentation tooling when relevant and available; ordinary prose edits do not require application builds, full test suites or deployment.
