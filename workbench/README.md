# Optimist Workbench

The Vue workbench is the interactive client for Optimist's typed systems model. It currently supports project selection and creation, archive download/upload, guided typed node creation, all eight typed relationship kinds, graph search/kind filters, Cytoscape navigation, and a typed inspector. Choose **New project...** in the project dropdown to create another model without leaving the active project until creation succeeds.

Node creation separates identity from simulation setup. Outcomes and factors are created with a current normalized-state estimate, interventions with duration and success assumptions, and metrics with a unit plus optional aggregation. Existing nodes with required or recommended gaps are marked consistently in the navigator, canvas, and inspector; inspector actions open the exact missing estimate editor. Required gaps mean scenario propagation is blocked when that node participates, while recommended intervention inputs improve projection fidelity without pretending the backend requires them.

The navigator's **Needs setup** filter turns those readiness markers into a review queue. It composes with text search and node-kind filters, so teams can focus on incomplete outcomes, factors, or interventions without losing the existing graph navigation workflow.

Select a node and use **Details** to edit its title, Markdown description, and JSON metadata with project/node revision guards. Factors and outcomes expose **Estimate** for current/desired normalized-state Point or Beta priors with provenance, plus revision-checked qualitative evidence records. Metric inspectors show each edge-scoped measurement history, append timestamped readings with optional known measurement error, and preserve immutable correction chains. Intervention inspectors edit typed duration and success estimates plus independent named cost dimensions. Scenario editing remains under development.

Graph layout reserves a top band for interventions and a bottom band for outcomes, leaving factors and metrics between them so causal direction remains legible in larger models. Selecting a node highlights every incident relationship and displays effect/degree summaries. A focused-relationship rail on the canvas and the inspector relationship list both provide large edit targets; direct edge taps also open the editor. Markdown description and JSON metadata use edge revision guards, and relationships can be deleted after explicit confirmation without deleting endpoint nodes. Causal effect and blocking estimates can be replaced with typed distributions; optional causal lags can be set or removed while mechanism and evidence context remains visible.

Dense graphs use semantic zoom automatically: overview hides labels and softens non-focused edges, context restores compact labels, and detail shows the complete graph vocabulary. Selected neighborhoods and analysis highlights remain legible at every level. The canvas layout control switches between causal hierarchy and deterministic kind clusters, with a cluster legend reporting visible actions, factors, metrics, and objectives.

The header command bar displays `Cmd+K` on Apple platforms and `Ctrl+K` elsewhere so its keyboard shortcut is discoverable before first use. It applies a deterministic typed grammar rather than natural language, provides context-aware completion, validates endpoint kinds and numeric bounds, shows the exact typed action and generated setup assumptions, and enables Apply only for a complete command:

```text
add factor "Fast feedback" controllable
add outcome "Reliable delivery" maximize
add metric "Cycle time" days
add intervention "Automate review"
connect A changes B 0.35
select B
mode optimize
```

Node commands use the same simulation-ready defaults as the creation wizard and retain an explicit provenance warning for later review. Relationship commands resolve an exact ID, semantic name, or quoted exact title. After a source is selected, autocomplete only offers relationship kinds which that node can originate and for which at least one compatible destination currently exists; destination completion applies the same endpoint rules. Duplicate or invalid relationships are still rejected before mutation. The command bar delegates apply to the existing typed node/edge APIs, so revision checks, persistence, replay, and mutation error handling remain unchanged.

Estimate editors render a live relative-density preview for Point, Beta, Scaled Beta, Normal, and LogNormal distributions. Accessible parameter popovers explain support, shape, spread, and tail behavior in operational terms. Popovers are portaled to the viewport so scrolling dialogs cannot clip them. Each typed estimate slot still limits selection to distribution families whose full support is valid for that quantity.

Every estimate editor also offers a **Fermi decomposition** assistant. Define named variables with compact estimates such as `1.5M` and human unit expressions such as `people/household`, then compose them with `+`, `-`, `*`, `/`, integer powers, and parentheses. New variables default to a LogNormal 90% interval from one tenth to ten times the entered estimate. Expand a variable only when a custom low/likely/high Beta-PERT range is justified. The assistant continuously evaluates the central arithmetic, composes and canonicalizes units, highlights malformed or unused variables, and reports residual dimensions before Monte Carlo is available.

The arithmetic source is the versioned `optimist_squiggle_v1` subset. After a short debounce, the lazily loaded Squiggle runtime translates the current variables and shows a fixed-seed prior-predictive preview with expected value, standard deviation, median, inner 50% band, and central 90% interval. It separately reports probability mass outside the slot's required support before clamping, and it leaves invalid negative mass visible for non-negative slots. Edits discard stale evaluations, so the preview follows the current equation without increasing the initial workbench bundle. This live result is explanatory only: adoption still requires the Rust API to validate and assess the canonical typed formula.

The Rust API validates the derived goal unit, runs deterministic Monte Carlo, reports convergence and rejected draws, and recommends a support-compatible effective distribution with a central 90% interval by matching sampled moments. Estimate source is exclusive: choose either **Distribution** or **Fermi equation**. Saving a Fermi source persists the equation, variables, canonical typed formula, sampling controls, diagnostics, and effective distribution in the estimate revision. Reopening the estimate restores those inputs for review; switching back to Distribution replaces the Fermi source. The server always reassesses submitted definitions and never accepts a client-supplied result distribution. Moment matching preserves mean and variance, not tails or multimodality.

Measurement relationships can now define explicit **metric-to-state calibration**. Higher/lower-is-better measurements map two metric readings to normalized factor states 0 and 1; target-range measurements use outer-zero and ideal-one anchors on both sides. Metric inspectors show the normalized state implied by each reading and correction. A factor's estimate editor lists the latest unsuperseded calibrated readings and can adopt one as a Point estimate only when the engineer chooses **Use reading**, retaining the reading, source, timestamp, and mapping result in provenance. Calibration does not perform an automatic Bayesian update and does not overwrite estimates when observations arrive.

The graph navigator provides synchronized Outline and Table views. Selecting a row updates the canvas and inspector; Arrow keys move one visible node at a time, while Home and End jump to the first and last visible nodes using roving keyboard focus.

**Feedback** mode runs Optimist's exact structural analysis against the active graph revision. It lists feedback strongly connected components and bounded elementary cycles, warns when cycle enumeration is truncated, and highlights a selected cycle's causal nodes and relationships on the canvas. This is topology review, not a statistical claim about loop stability.

**Optimize** mode creates and revision-edits finite-horizon scenarios from explicit outcome objectives and candidate interventions, then projects each candidate independently through the real Monte Carlo analysis API. A styled, keyboard-accessible scenario menu displays scenario ID, revision, and horizon and is portaled outside the scrolling panel. Results retain objective-level improvement, Monte Carlo standard error, reachability, convergence, valid/invalid draw counts, clamping diagnostics, and seed. It does not rank candidates, enforce budgets, evaluate bundles, or apply conflicts and synergies; those remain separate roadmap work.

**Impediments** mode projects factors with causal paths to outcomes. Topology ordering uses reachable-outcome count and shortest distance; Evidence ordering separately prioritizes direct node evidence and typed relationship references. Candidate cards expose controllability, path coverage, and unsupported path edges, and selecting one highlights the exact reviewed path. Neither ordering is a causal confidence score or effect estimate.

Use the header download button to save the selected project as `.optimist.json`. The upload button previews archive identity/counts before restore. Replacing an existing project requires typing its project ID. Imported archives start a new replay lineage at their archived revision because portable archives do not carry server event logs. Native server snapshots retain complete projects, allocator positions, committed changes, and idempotent command results across ordinary restarts. A write-ahead command journal idempotently completes commands interrupted around snapshot publication without duplicating graph state or replay events. Known older server snapshots migrate forward at startup only after integrity validation succeeds.

REST and WebSocket replay automatically provide a canonical project snapshot when a client's cursor predates retained history. Clients replace local project state at the supplied revision before applying later live events.

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

`test:e2e` uses deterministic mocked API state for desktop/mobile layout, screenshots, command completion/diagnostics/preview/apply, intervention-to-outcome hierarchy, semantic zoom transitions, deterministic kind clusters, dense-graph viewport containment, readiness markers and setup wizard, focused relationship metadata/editing, analysis-mode highlighting, impediment ordering, scenario projection results, canvas-pixel checks, and a 100-node render bound. `test:e2e:real` starts Axum and Vite on isolated ports and verifies command-bar mutations, exact structural feedback analysis, evidence-aware impediment projection, finite-horizon scenario analysis, direct relationship editing, estimate lifecycles, evidence records, observation correction chains, edge/node deletion, archive download, mutation, and confirmed restore through the real proxy. Both Playwright configurations use a non-interactive line reporter and exit after completion.

Playwright scenarios are organized one per file under domain directories. Mocked workflows live in `tests/{analysis,graph,performance,projects,responsive}` with reusable routing/canvas helpers in `tests/support`. Real-Axum workflows live in `tests-real/{analysis,estimates,graph,observations,projects}` with UI setup helpers in `tests-real/support`; every real scenario creates its own isolated project.

Component-specific CSS lives in a scoped `<style>` block beside the Vue template and behavior it supports. `src/style.css` is intentionally limited to design tokens, element resets, accessibility utilities, and primitives shared across unrelated components such as buttons, dialogs, form grids, estimate rows, and analysis-panel structure. Teleported menus and dialogs keep their styles in the owning component; use `:deep()` only when an owner intentionally styles a child component's public layout surface.
