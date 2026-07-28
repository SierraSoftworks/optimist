---
name: optimist-design-review
description: Validate and evaluate changes to an Optimist system design with the `optimist` CLI. Use when editing a design directory (`_system.yaml`, `components/*.yaml`, `component-types/*.yaml`, `mutators/*.yaml`), when asked whether a capacity model is correct, what a system's bottleneck is, whether an intervention or proposal helps, or after changing component properties, relationships, behaviours, scale units, or interventions. Covers `optimist check`, `solve`, `bottlenecks`, `compare`, and `catalogue`, and how to read what they report.
---

# Reviewing a change to an Optimist design

A design is a directory of YAML describing a system's capacity: components, the
relationships between them, and the limits each one can exhaust. Editing it is
easy; knowing whether the edit meant anything is not. The engine absorbs most
mistakes silently — a misspelled property is ignored, an intervention that
rebinds nothing compares identically against itself — so a design that solves
and produces plausible numbers may be answering a question nobody asked.

Never claim a change improved a design without running `compare`. A capacity
model exists precisely because intuition about queues and utilisation is
unreliable.

## Always finish an edit with this loop

```sh
optimist check <design>        # 1. does it mean what was written?
optimist bottlenecks <design>  # 2. what does it run out of first?
optimist compare <design> <intervention>   # 3. does the proposal help?
```

Run them from the repository root, naming the design directory. Every command
except `compare` defaults to the working directory, so `optimist check` alone
works when standing inside a design.

Use `--output json` when the answer is going to be reasoned over rather than
shown to the user, and read the table form when reporting back.

## 1. `optimist check` — before anything else

```sh
optimist check <design>
```

**Exit status is the signal.** Non-zero means at least one `error` finding and
the design is not fit to solve. Warnings alone exit zero but almost always
indicate a mistake worth fixing.

Findings it reports that nothing else will:

| Finding | Severity | What it actually means |
| --- | --- | --- |
| sets `X`, which `<type>` does not declare | `warning` | Misspelled property. **The value is being ignored.** |
| does not supply `X`, which `<type>` requires | `error` | The design cannot solve. |
| adopts the unknown type `<type>` | `error` | Wrong `type:` or a missing manifest. |
| attaches the unknown behaviour `<id>` | `error` | Wrong `type:` under a relationship's `mutators:`. |
| rebinds `X`, which is not a shared quantity | `error` | Interventions may only rebind `scratchpad` entries. |
| rebinds nothing | `warning` | The intervention is a no-op. |
| is not wired to anything | `warning` | The component contributes no load. |
| did not settle after N passes | `warning` | **No steady state exists. Every number is meaningless.** |

`--no-solve` skips the trial solve when only the structure is in question.

For JSON, `solvable` is the same condition as the exit status:

```sh
optimist --output json check <design> | jq '.solvable, .findings'
```

### When a design does not settle

This is a modelling error, not a capacity result. It means a feedback loop has
gain above one. The usual cause is a component on the *response* leg of a loop
publishing `rate`, which feeds demand back into its own caller. Look at the
relationships first, not the property values.

## 2. `optimist catalogue` — before writing a component

```sh
optimist catalogue <design>                # what is available, and what is used
optimist catalogue <design> --type compute # properties, ports, channels, limits
```

Read `--type` before authoring or editing a component rather than guessing at
property names. It lists which properties are `required`, the unit each carries,
every channel the type derives, and every constraint it can exhaust. It works
for behaviours too.

Note the `IN USE` column: a type nothing uses is a candidate for deletion, and a
type used everywhere is one to change carefully.

## 3. `optimist bottlenecks` — what the design runs out of

```sh
optimist bottlenecks <design> --binding
```

Rows are ordered by how likely a constraint is to bind, not by mean load,
because the constraint most exposed to a bad draw is the one worth spending on.

| Column | How to read it |
| --- | --- |
| `MEAN` | Demand over limit. Above `1` the constraint is exhausted on average. |
| `P90` | The same at the ninetieth percentile of draws. |
| `BINDS` | Share of draws in which demand met or exceeded the limit. |
| `HEADROOM` | Limit less demand, in the constraint's own units. Negative means over. |

A constraint at `MEAN` 0.6 that `BINDS` in 20% of draws is a real problem: the
mean is not the number to plan against. Report both.

`--component <id>` narrows to one component; `--intervention <id>` ranks the
design as it would be with a proposal applied.

## 4. `optimist compare` — whether a proposal helps

```sh
optimist compare <design> <intervention> [<intervention>...]
```

Both sides are solved with the same seed and the same draws, so every difference
is attributable to what the intervention rebound. Name several proposals and
they become comparable with each other as well as with the baseline.

The note under each table says which of three things happened:

- **relieves what it was aimed at** — constraints stopped binding and none
  started. Still check what it says is *"still binding afterwards"*: relieving
  one limit while the design remains short elsewhere is not a fix.
- **moves the bottleneck** — something started binding that did not before.
  This is the common outcome. Say so plainly rather than reporting the
  improvement alone.
- **changes nothing that binds** — the proposal moved quantities that were
  never what the design was short of.

An intervention only rebinds `scratchpad` entries. If a change cannot be
expressed that way, the quantity it acts on has not been named yet — lift it
into the scratchpad first. That is a modelling improvement in its own right.

## 5. `optimist solve` — when a number needs explaining

```sh
optimist solve <design> --component <id>
```

Reach for this to explain *why* a constraint binds, not to check whether one
does. A component's own channels come first, then the traffic on its ports:
`in.<port>.<signal>` is what arrived, `out.<port>.<signal>` is what came back.
A component's latency and the dependency latency that caused it are therefore
in the same table.

Uncertain quantities carry a central eighty percent interval. A wide interval
is information: it means the design's behaviour is not determined by what is
known about it, and quoting the mean alone would be dishonest.

## Determinism and cost

- Results are deterministic for a given `--seed` and `--samples`. Do not compare
  two runs with different values of either and attribute the difference to a
  change.
- `--samples 1000` is the default. Raise it when a probability near zero or one
  matters; lower it when iterating.
- `--transient` advances queues through time instead of solving for balance.
  Use it only to watch a design recover from a surge, and shorten `--step` while
  lengthening `--horizon` together when you do.

## Reporting back

State, in this order: whether `check` passed, what binds first and how hard,
and what each proposal did — including what it moved the problem *to*. Quote
the constraint as `component.constraint`, give both the mean and the share of
draws that bind, and name the units when quoting headroom. If the design did
not settle, say that and stop; nothing downstream of it is worth reporting.
