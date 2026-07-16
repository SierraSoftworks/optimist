# Markdown document format

Optimist defines a deterministic, versioned Markdown representation for projects, graph entities, outgoing edges, scenarios, dependence, and formulas.

## Directory layout

```text
model/
├── _project.md
├── entities/
│   ├── A-reliable-delivery.md
│   ├── B-fast-feedback.md
│   └── C-small-batches.md
└── scenarios/
    └── A-next-quarter.md
```

Paths are canonical and deterministic. Entity descriptions use the Markdown body of their owning file. Outgoing edges are stored in the source entity's frontmatter. Scenario rationale uses the scenario Markdown body.

## Project document

`_project.md` stores:

- schema version,
- project identity and base revision,
- optional dependence document,
- revisioned formula document,
- project Markdown description.

Every document in one import snapshot must declare the same base project revision.

## Parser guarantees

The parser:

- accepts UTF-8 with canonical LF line endings,
- requires exact `---` frontmatter delimiters,
- limits complete document and YAML frontmatter sizes independently,
- rejects unknown schema versions and unknown YAML fields,
- reports path, line, and column for YAML failures,
- validates local node/edge/scenario/dependence invariants.

A second validation pass resolves cross-file references, unique IDs/names/aliases, scenario node kinds, dependence estimate addresses, formula primitive roots, parent components, references, cycles, and units.

## Rendering guarantees

Rendering is deterministic:

- stable field ordering,
- canonical paths,
- outgoing edges sorted by canonical ID,
- LF endings,
- parse-render-parse semantic stability.

`RenderedSnapshot` creates an ordered in-memory file map. Directory publication writes a complete sibling staging directory and uses backup-and-rollback replacement, removing stale generated files after success.

::: warning Atomicity boundary
Portable `std::fs` cannot atomically replace an existing non-empty directory on every supported filesystem. Optimist stages all files before publication and attempts rollback, but the stronger cross-platform atomic-directory guarantee remains open.
:::

## Merge planning

A validated imported snapshot can be compared with a current snapshot without mutation. The deterministic plan reports:

- `Create`,
- `Update`,
- `Unchanged`,
- `Conflict`.

Semantic equality ignores stale base revisions when content is unchanged. Changed content from a stale base conflicts rather than overwriting concurrent work. Different projects and conflicting aggregate revisions also produce explicit conflicts.

## Current transport status

The Rust library supports parsing, rendering, validation, merge planning, directory reading, and staged publication. `optimist project import|export` and HTTP transport are not implemented yet; those CLI commands return actionable unavailable errors.
