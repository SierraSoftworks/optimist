# YAML project format

Optimist uses one deterministic, versioned YAML structure for portable projects and authoritative project snapshots. Projects do not embed Markdown files, generated samples, effective distributions, or derived percentiles.

## Directory layout

```text
model/
├── _project.yaml
├── entities/
│   ├── A-customer_impact.yaml
│   ├── B-fast_feedback.yaml
│   └── C-small_batches.yaml
└── scenarios/
    └── A-next_quarter.yaml
```

Paths are canonical. Slugs use normalized lower-case words separated by underscores. Each entity file contains its complete node and every outgoing edge whose `source` is that node. Scenario files contain complete scenario documents.

## Project document

`_project.yaml` stores:

- `schema_version`,
- project identity, name, and revision,
- optional project description,
- optional residual dependence metadata.

Every entity and scenario declares the same `base_project_revision`.

## Estimates

Squiggle is the only persisted distribution and mathematical-expression format. An estimate stores authored source and deterministic controls:

```yaml
source:
  type: squiggle
  definition:
    source: |-
      baseline = Sym.beta(8, 2)
      disruption = mixture([pointMass(0), Sym.beta(2, 8)], [0.8, 0.2])
      baseline * (1 - disruption)
    seed: 42
    sample_count: 2048
    target_unit: {}
```

The effective distribution, generated samples, moments, and quantiles are absent. Optimist reevaluates the source with the stored seed when runtime analysis needs them. Supported symbolic results remain analytical; composed distributions use deterministic in-memory draws only for the duration of analysis.

## Validation

The parser:

- accepts UTF-8 with canonical LF line endings,
- bounds each document and the complete project,
- rejects unknown fields and unsupported schema versions,
- validates node, edge, scenario, estimate, and dependence invariants,
- rejects duplicate IDs, names, aliases, and paths,
- resolves edge endpoints, scenario references, and dependence addresses across files,
- rejects Markdown project files instead of importing them.

Rendering is deterministic: stable field order, canonical paths, sorted outgoing edges, and LF endings. Directory publication writes a complete sibling staging directory before replacing the destination, so stale generated files are removed only after the new project has been rendered successfully.

## Browser transport

The workbench downloads one `.optimist.yaml` file containing the same typed structure as the split directory form. Counts shown before import are derived from `entities`, their `outgoing_edges`, and `scenarios`; they are not serialized metadata.

Replacing an existing project requires explicit confirmation. Validation and fresh repository construction complete before the catalog entry is swapped. Portable projects do not carry event history, so replay starts at the imported revision.