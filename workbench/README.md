# Optimist Workbench

The Vue workbench is the interactive client for Optimist's typed systems model. It currently supports project selection and creation, archive download/upload, typed node creation, all eight typed relationship kinds, graph search/kind filters, Cytoscape navigation, and a typed inspector. Choose **New project...** in the project dropdown to create another model without leaving the active project until creation succeeds.

Select a node and use **Details** to edit its title, Markdown description, and JSON metadata with project/node revision guards. Factors and outcomes expose **Estimate** for current/desired normalized-state Point or Beta priors with provenance, plus revision-checked qualitative evidence records. Metric inspectors show each edge-scoped measurement history, append timestamped readings with optional known measurement error, and preserve immutable correction chains. Intervention inspectors edit typed duration and success estimates plus independent named cost dimensions. Scenario editing remains under development.

Incident relationships in the inspector are selectable. Their Markdown description and JSON metadata can be edited with edge revision guards, and relationships can be deleted after an explicit confirmation without deleting endpoint nodes. Causal effect and blocking estimates can be replaced with typed distributions; optional causal lags can be set or removed while mechanism and evidence context remains visible.

Estimate editors render a live relative-density preview for Point, Beta, Scaled Beta, Normal, and LogNormal distributions. Accessible parameter popovers explain support, shape, spread, and tail behavior in operational terms. Each typed estimate slot still limits selection to distribution families whose full support is valid for that quantity.

The graph navigator provides synchronized Outline and Table views. Selecting a row updates the canvas and inspector; Arrow keys move one visible node at a time, while Home and End jump to the first and last visible nodes using roving keyboard focus.

Use the header download button to save the selected project as `.optimist.json`. The upload button previews archive identity/counts before restore. Replacing an existing project requires typing its project ID; replacement discards current process-local replay history after the canonical Markdown snapshot validates successfully.

## Run locally

Start the Optimist API from the repository root:

```sh
cargo run -- server --bind 127.0.0.1:3000
```

Then start Vite:

```sh
cd workbench
npm install
npm run dev
```

Vite proxies `/api` to `http://127.0.0.1:3000`. Set `OPTIMIST_API_URL` before `npm run dev` to use another server.

## Validate

```sh
npm test
npm run build
npm run test:e2e -- --workers=1
npm run test:e2e:real
npm audit
```

`test:e2e` uses deterministic mocked API state for desktop/mobile layout, screenshots, canvas-pixel checks, and a 100-node render bound. `test:e2e:real` starts Axum and Vite on isolated ports and verifies representative relationships, edge and node estimate lifecycles, evidence records, observation correction chains, edge/node deletion, archive download, mutation, and confirmed restore through the real proxy. Both Playwright configurations use a non-interactive line reporter and exit after completion.
