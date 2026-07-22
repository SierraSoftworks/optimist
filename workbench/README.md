# Optimist Workbench

The Vue workbench is the interactive client for Optimist's typed systems model. It currently supports project selection and creation, archive download/upload, guided typed node creation, all eight typed relationship kinds, graph search/kind filters, Cytoscape navigation, and a typed inspector. Choose **New project...** in the project dropdown to create another model without leaving the active project until creation succeeds.

Node creation separates identity and quantity metadata from uncertain estimates. Outcomes and factors start without a current estimate, interventions start without duration or success assumptions, and metrics start without a current native-unit estimate. The readiness markers in the navigator, canvas, and inspector identify required or recommended gaps; each inspector action opens a Squiggle editor for the exact missing slot. Required gaps mean scenario propagation is blocked when that node participates, while recommended intervention inputs improve projection fidelity without pretending the backend requires them.

The navigator's **Needs setup** filter turns those readiness markers into a review queue. It composes with text search and node-kind filters, so teams can focus on incomplete outcomes, factors, or interventions without losing the existing graph navigation workflow.

Select a node and use **Details** to edit its title, Markdown description, and JSON metadata with project/node revision guards. Factors and outcomes expose **Estimate** for current or desired normalized state, metrics use the same editor in their native unit, and intervention inspectors use it for duration, success probability, and named cost dimensions. Every slot accepts direct Squiggle source and provenance; there are no separate distribution-family or formula builders. Metric inspectors also show their quantity contract and edge-scoped measurement histories, append timestamped readings with optional known measurement error, and preserve immutable correction chains. Scenario editing remains under development.

Graph layout reserves a top band for interventions and a bottom band for outcomes, leaving factors and metrics between them so causal direction remains legible in larger models. Selecting a node highlights every incident relationship and displays effect, response, or degree summaries. A focused-relationship rail on the canvas and the inspector relationship list both provide large edit targets; direct edge taps also open the editor. Markdown description and JSON metadata use edge revision guards, and relationships can be deleted after explicit confirmation without deleting endpoint nodes. Normalized causal effects, native destination responses, blocking estimates, and optional causal lags use the same Squiggle source workflow while mechanism and evidence context remains visible.

Metrics participate in `contributes` relationships as both sources and destinations. A metric-touching relationship asks for a counterfactual source change and expected destination change in the endpoint units, then displays the pair instead of an opaque signed strength. The backend rejects missing canonical dimensions, zero source anchors, normalized effects on metric paths, and response units which disagree with the nodes. Scenario propagation samples the resulting local coefficient and clamps each state to its own support: normalized states to `[0, 1]`, non-negative metrics at zero, bounded metrics to their interval, and real metrics not at all. Intervention `changes` remains normalized factor-only until direct native intervention-shift semantics are specified.

Dense graphs use semantic zoom automatically: overview hides labels and softens non-focused edges, context restores compact labels, and detail shows the complete graph vocabulary. Selected neighborhoods and analysis highlights remain legible at every level. The canvas layout control switches between causal hierarchy and deterministic kind clusters, with a cluster legend reporting visible actions, factors, metrics, and objectives.

The header command bar displays `Cmd+K` on Apple platforms and `Ctrl+K` elsewhere so its keyboard shortcut is discoverable before first use. It applies a deterministic typed grammar rather than natural language, provides context-aware completion, validates endpoint kinds and numeric bounds, shows the exact typed action and remaining estimate setup, and enables Apply only for a complete command:

```text
add factor "Fast feedback" controllable
add outcome "Reliable delivery" maximize
add metric "Cycle time" days
add intervention "Automate review"
connect A changes B 0.35
select B
mode optimize
```

Node commands create the same metadata-only nodes as the creation wizard, and readiness immediately exposes any estimate work still required. Relationship commands resolve an exact ID, semantic name, or quoted exact title. After a source is selected, autocomplete only offers relationship kinds which that node can originate and for which at least one compatible destination currently exists; destination completion applies the same endpoint rules. Duplicate or invalid relationships are still rejected before mutation. The command bar delegates apply to the existing typed node/edge APIs, so revision checks, persistence, replay, and mutation error handling remain unchanged.

Every estimate editor is a direct Squiggle source editor. Starter calculations reflect the slot support, but users can replace them with any runtime expression whose final value is a finite number or sampleable distribution. This includes symbolic families, arithmetic over distributions, mixtures, truncation, and `SampleSet` operations. The same editor serves normalized states, native metric quantities, intervention assumptions, costs, causal effects, native responses, and lags.

After a short debounce, the workbench sends source, seed, retained sample count, and owner-derived target unit to Optimist's Rust Squiggle runtime. The backend lints and evaluates the wrapped unit annotation, then returns family, mean, variance, median, and a central 90% interval. Browser code does not load a second Squiggle runtime and does not synthesize an estimate result. Stale responses are discarded, and source outside the slot's empirical support keeps Save disabled.

Saving uses the same backend evaluator as preview. Optimist persists the authored source and deterministic controls, a backend-generated assessment, and an effective distribution for existing analysis code. Distribution-valued programs retain 256 to 4,096 deterministic draws, 2,048 by default, so multimodal and transformed results do not have to be forced into a primitive family. Reopening restores the original source. Deserialization reevaluates it and rejects persisted assessments or draws which no longer match. Legacy direct-distribution and Fermi estimates remain readable; opening one translates its effective distribution to Squiggle, and saving replaces the legacy authoring source.

Measurement relationships can now define explicit **metric-to-state calibration**. Higher/lower-is-better measurements map two metric readings to normalized factor states 0 and 1; target-range measurements use outer-zero and ideal-one anchors on both sides. Metric inspectors show the normalized state implied by each reading and correction. A factor's estimate editor lists the latest unsuperseded calibrated readings and can adopt one as a Squiggle `pointMass` only when the engineer chooses **Use reading**, retaining the reading, source, timestamp, and mapping result in provenance. Calibration does not perform an automatic Bayesian update and does not overwrite estimates when observations arrive.

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
