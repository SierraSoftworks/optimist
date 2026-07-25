# Modelling systems

Optimist separates structural graph concepts from values which have one natural owner. This keeps causal traversal focused while preserving evidence and uncertainty alongside the thing they describe.

## Node kinds

| Kind | Meaning | Typical owned data |
| --- | --- | --- |
| Outcome | A result whose direction guides prioritisation. | Native quantity, current/forecast estimates, evidence. |
| Metric | A directly measurable native-unit quantity. | Operational definition, support, current estimate, observations. |
| Factor | A condition influencing another part of the system. | Native quantity, current/forecast estimates, controllability, evidence. |
| Intervention | An investable action. | Costs, duration, probability of success, acceptance criteria. |

Names and aliases are unique within a project after Unicode normalisation, lowercasing, and whitespace collapse. IDs are compact counters (`A`, `B`, ..., `BA`) scoped to their project.

## Native quantities

A metric defines a quantity independently of whether its movement is desirable. Its unit, aggregation window, legal support, operational definition, reference time, and resolution source answer what value is being estimated and how it could eventually be checked. Maximize, minimize, and target-range preferences belong to scenarios rather than the metric.

Metric support is one of:

- any finite real value,
- zero or greater,
- an inclusive finite interval.

The metric's optional current estimate is expressed directly in that native unit. A latency metric can therefore retain a Squiggle LogNormal calculation in days. Metric storage contains one nested quantity definition plus its optional estimate.

Factors and outcomes use the same native quantity contract. Their state owns one quantity definition, an uncertain current estimate, and an optional pre-intervention forecast. Scenario analysis uses the forecast when present, otherwise current, and clamps simulated values only to the declared support. Configure the quantity before authoring either estimate or creating a causal relationship.

Every persisted estimate retains Squiggle source. The owner determines the target unit and legal support: fresh real quantities start with a Normal calculation, non-negative quantities with LogNormal, and bounded quantities with an affine Beta on the declared interval. These are editable starting points rather than mandatory families. Optimist evaluates source in Rust, preserves supported symbolic families, and creates deterministic empirical draws only in memory for composed results.

Every estimate can retain provenance plus separate descriptions of epistemic uncertainty (knowledge and model gaps), process uncertainty (variation between realizations), and measurement uncertainty (observation and resolution error). These descriptions are reviewable assumptions alongside the authoritative total distribution. Optimist does not assign numeric shares, add component variances, or assume that the categories are independent.

## Edge kinds

| Kind | Direction | Purpose |
| --- | --- | --- |
| `contributes` | Directed | A proportional causal response. |
| `measures` | Directed | A metric measuring a factor or outcome. |
| `changes` | Directed | An intervention changing a factor or native metric. |
| `requires` | Directed | A hard or soft prerequisite. |
| `part-of` | Directed | Non-causal factor decomposition. |
| `blocks` | Directed | A factor inhibiting a factor or intervention. |
| `conflicts-with` | Symmetric | Incompatible interventions. |
| `synergizes-with` | Symmetric | Mutually beneficial interventions. |

Endpoint combinations are validated. `contributes` may connect any configured factor, metric, or outcome to another such state variable. Every causal edge carries one dimensionless response estimate, so the endpoints' units need not agree and re-baselining either endpoint leaves the claim intact. `measures` remains a metric-to-factor/outcome observation model.

Canonical edge IDs use `<source>-<kind>-<destination>`, such as `B-requires-A`. Symmetric edges are ordered by entity ID so both input orders produce one identity.

## Embedded ownership

Optimist deliberately does not create graph vertices for:

- estimates,
- observations,
- intervention costs,
- evidence.

An observation belongs to one `measures` edge because the same metric may measure several subjects with independent histories. A cost belongs to one intervention. A causal response belongs to one causal edge.

This design avoids graph noise and makes deletion/reference checks explicit.

## Causal responses

Every `contributes` relationship answers a concrete counterfactual:

> If the source moves by some fraction of its baseline, what fraction of its own baseline should we expect the destination to move after the stated lag?

That ratio of fractional changes is an elasticity $\varepsilon_{xy}$, and it carries no unit:

$$
\varepsilon_{xy} = \frac{\Delta y / y_0}{\Delta x / x_0}.
$$

So $\varepsilon = 1$ is a plain product, $\varepsilon = 0$ is no response, and a negative value inverts the direction. Because it is dimensionless, one relationship can connect a rate to a duration without the author converting between them.

A `changes` relationship is different, because an intervention has no level to take a ratio of. Its response is instead the multiplier applied to the target while the intervention is fully active: `0.1` cuts the target to a tenth, `1` leaves it unchanged, and `1.25` raises it by a quarter.

Both are revisioned Squiggle estimates evaluated as dimensionless quantities, so uncertainty in the response is expressed the same way as anywhere else in the model.

This response is a modelling claim, not causal identification from observed correlation. Mechanism, assumptions, and evidence remain explicit edge context. Observational co-movement should not be promoted to a response without an experiment or documented identification argument.

## Time-boxed interventions

Interventions are permanent by default: once an effect arrives it holds for the rest of the horizon. Many real interventions are not. A change freeze runs for two planning cycles, a temporary staffing uplift ends when the contract does, and a mitigation gets reverted once the underlying defect is fixed.

Give a `changes` relationship an effect profile to say how long it lasts:

```sh
cargo run -- --project A batch apply \
  --request-id 00000000-0000-4000-8000-000000000010 \
  --expected-revision 12 \
  --commands '[{
    "type": "set_effect_profile",
    "payload": {
      "edge": {"source": "H", "kind": "changes", "destination": "E"},
      "expected_revision": 0,
      "profile": {
        "ramp": null,
        "hold": {"source": "pointMass(2)", "seed": 42, "sample_count": 256, "target_unit": {"duration": 1}},
        "release": {"type": "immediate"},
        "aftereffect": {
          "magnitude": {"source": "pointMass(1.25)", "seed": 42, "sample_count": 256, "target_unit": {}},
          "hold": {"source": "pointMass(1)", "seed": 42, "sample_count": 256, "target_unit": {"duration": 1}},
          "release": {"type": "immediate"}
        }
      }
    }
  }]'
```

That reads as: hold the effect for two periods, end it abruptly, and run the change rate a quarter above baseline for one period afterwards as the suppressed work drains. Set `profile` to `null` to restore a permanent effect.

The rebound declares its own multiplier rather than a share of the effect it follows, so releasing an intervention can overshoot, undershoot, or exactly reverse. Every duration is a Squiggle estimate, so an uncertain schedule is expressed the same way as any other uncertain quantity. `ramp` staggers the onset, and `release` can decline linearly over a span or decay by half-life instead of stopping abruptly.

Only `changes` accepts a profile. A `contributes` relationship describes an ongoing structural dependency with no activation to start or stop, and Optimist rejects a profile on one.

## Descriptions and metadata

Nodes and edges carry Markdown descriptions plus extensible JSON metadata. Updates are complete replacements guarded by the aggregate revision:

```sh
cargo run -- node update B \
  --title "Fast feedback" \
  --description $'# Fast feedback\n\nTime from change to useful evidence.' \
  --metadata '{"owner":"platform"}'

cargo run -- edge update C-part-of-B \
  --description $'# Decomposition\n\nSmall batches are one part of fast feedback.' \
  --metadata '{"source":"ADR-17"}'
```

These commands preserve identity, names, aliases, endpoint kinds, typed payloads, estimates, and observation histories.

## Project documents

Some concepts span several graph aggregates and therefore live outside the graph:

- **Scenarios** define objectives, horizon, budgets, candidates, and sampling controls.
- **Dependence documents** group residual marginals under Gaussian copulas.

Each document has its own revision. Structural analysis keys include the graph, scenario, and dependence revisions, making the input snapshot explicit.

## Choosing model detail

Start with the smallest graph which can answer the question:

1. Define one or more outcomes.
2. Add factors with a plausible direct causal mechanism.
3. Add metrics only where observations can be supplied.
4. Add interventions only when a team can choose or fund them.
5. Decompose uncertain quantities with named Squiggle bindings rather than creating structural nodes for arithmetic.
6. Add dependence only when shared causes or residual correlation are justified.

A denser graph is not automatically a better model. Every causal edge should have a mechanism, evidence boundary, and uncertainty model that can be reviewed.
