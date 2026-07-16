# Optimist Workbench

The Vue workbench is the interactive client for Optimist's typed systems model. It currently supports project selection and creation, archive download/upload, typed node creation, all eight typed relationship kinds, graph search/kind filters, Cytoscape navigation, and a typed inspector. Choose **New project...** in the project dropdown to create another model without leaving the active project until creation succeeds.

Select a node and use **Details** to edit its title, Markdown description, and JSON metadata with project/node revision guards. Factors and outcomes expose **Estimate** for current/desired normalized-state Point or Beta priors with provenance, plus revision-checked qualitative evidence records. Metric inspectors show each edge-scoped measurement history, append timestamped readings with optional known measurement error, and preserve immutable correction chains. Intervention inspectors edit typed duration and success estimates plus independent named cost dimensions. Scenario editing remains under development.

Incident relationships in the inspector are selectable. Their Markdown description and JSON metadata can be edited with edge revision guards, and relationships can be deleted after an explicit confirmation without deleting endpoint nodes. Causal effect and blocking estimates can be replaced with typed distributions; optional causal lags can be set or removed while mechanism and evidence context remains visible.

Estimate editors render a live relative-density preview for Point, Beta, Scaled Beta, Normal, and LogNormal distributions. Accessible parameter popovers explain support, shape, spread, and tail behavior in operational terms. Popovers are portaled to the viewport so scrolling dialogs cannot clip them. Each typed estimate slot still limits selection to distribution families whose full support is valid for that quantity.

Every estimate editor also offers a **Fermi decomposition** assistant. Define named variables with compact estimates such as `1.5M` and human unit expressions such as `people/household`, then compose them with `+`, `-`, `*`, `/`, integer powers, and parentheses. New variables default to a LogNormal 90% interval from one tenth to ten times the entered estimate. Expand a variable only when a custom low/likely/high Beta-PERT range is justified. The assistant continuously evaluates the central arithmetic, composes and canonicalizes units, highlights malformed or unused variables, and reports residual dimensions before Monte Carlo is available.

The Rust API validates the derived goal unit, runs deterministic Monte Carlo, reports convergence and rejected draws, and recommends a support-compatible effective distribution with a central 90% interval by matching sampled moments. Estimate source is exclusive: choose either **Distribution** or **Fermi equation**. Saving a Fermi source persists the equation, variables, canonical typed formula, sampling controls, diagnostics, and effective distribution in the estimate revision. Reopening the estimate restores those inputs for review; switching back to Distribution replaces the Fermi source. The server always reassesses submitted definitions and never accepts a client-supplied result distribution. Moment matching preserves mean and variance, not tails or multimodality.

Measurement relationships can now define explicit **metric-to-state calibration**. Higher/lower-is-better measurements map two metric readings to normalized factor states 0 and 1; target-range measurements use outer-zero and ideal-one anchors on both sides. Metric inspectors show the normalized state implied by each reading and correction. A factor's estimate editor lists the latest unsuperseded calibrated readings and can adopt one as a Point estimate only when the engineer chooses **Use reading**, retaining the reading, source, timestamp, and mapping result in provenance. Calibration does not perform an automatic Bayesian update and does not overwrite estimates when observations arrive.

The graph navigator provides synchronized Outline and Table views. Selecting a row updates the canvas and inspector; Arrow keys move one visible node at a time, while Home and End jump to the first and last visible nodes using roving keyboard focus.

**Feedback** mode runs Optimist's exact structural analysis against the active graph revision. It lists feedback strongly connected components and bounded elementary cycles, warns when cycle enumeration is truncated, and highlights a selected cycle's causal nodes and relationships on the canvas. This is topology review, not a statistical claim about loop stability.

**Optimize** mode creates and revision-edits finite-horizon scenarios from explicit outcome objectives and candidate interventions, then projects each candidate independently through the real Monte Carlo analysis API. A styled, keyboard-accessible scenario menu displays scenario ID, revision, and horizon and is portaled outside the scrolling panel. Results retain objective-level improvement, Monte Carlo standard error, reachability, convergence, valid/invalid draw counts, clamping diagnostics, and seed. It does not rank candidates, enforce budgets, evaluate bundles, or apply conflicts and synergies; those remain separate roadmap work.

**Impediments** mode projects factors with causal paths to outcomes. Topology ordering uses reachable-outcome count and shortest distance; Evidence ordering separately prioritizes direct node evidence and typed relationship references. Candidate cards expose controllability, path coverage, and unsupported path edges, and selecting one highlights the exact reviewed path. Neither ordering is a causal confidence score or effect estimate.

Use the header download button to save the selected project as `.optimist.json`. The upload button previews archive identity/counts before restore. Replacing an existing project requires typing its project ID; replacement discards current process-local replay history after the canonical Markdown snapshot validates successfully. The server also writes complete project snapshots under `--data-dir`, so projects and allocator positions survive ordinary restarts even though replay history does not yet.

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

For a production-style single-process run, build the workbench and start Optimist from the repository root:

```sh
npm run build
cd ..
cargo run -- server
```

The server auto-discovers `workbench/dist`; `--web-root` and `OPTIMIST_WEB_ROOT` can select another build. HTML uses revalidation, hashed `/assets` are immutable, unknown API routes stay JSON errors, and non-API browser routes use SPA fallback.

## Validate

```sh
npm test
npm run build
npm run test:e2e -- --workers=1
npm run test:e2e:real
npm audit
```

`test:e2e` uses deterministic mocked API state for desktop/mobile layout, screenshots, analysis-mode highlighting, impediment ordering, scenario projection results, canvas-pixel checks, and a 100-node render bound. `test:e2e:real` starts Axum and Vite on isolated ports and verifies exact structural feedback analysis, evidence-aware impediment projection, finite-horizon scenario analysis, representative relationships, estimate lifecycles, evidence records, observation correction chains, edge/node deletion, archive download, mutation, and confirmed restore through the real proxy. Both Playwright configurations use a non-interactive line reporter and exit after completion.

Playwright scenarios are organized one per file under domain directories. Mocked workflows live in `tests/{analysis,graph,performance,projects,responsive}` with reusable routing/canvas helpers in `tests/support`. Real-Axum workflows live in `tests-real/{analysis,estimates,graph,observations,projects}` with UI setup helpers in `tests-real/support`; every real scenario creates its own isolated project.
