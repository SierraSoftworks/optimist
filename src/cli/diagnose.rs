//! Looking a design over for the mistakes its schema cannot catch.
//!
//! Reading a design already rejects anything malformed: a wrong schema version,
//! a relationship pointing at nothing, a component type that does not validate.
//! What survives that is a design which parses and still does not say what its
//! author meant — a property spelled differently from the one its type
//! declares, an intervention rebinding a quantity nobody named, a component
//! wired to nothing at all.
//!
//! None of those are errors to the engine. Each is silently absorbed: the
//! stray property is ignored, the intervention rebinds nothing and compares
//! identically against itself, the orphaned component contributes no load. The
//! design solves, the numbers look plausible, and the answer is to a question
//! nobody asked. Finding them is the point of `check`.

use std::collections::BTreeSet;

use crate::system::{EvaluationConfig, LoadedSystem, Solve};

/// How much a finding matters.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum Severity {
    /// The design will not solve, or something it states plainly has no effect.
    Error,
    /// The design solves, but part of it is probably not what was intended.
    Warning,
}

/// One thing worth telling the author about.
#[derive(Clone, Debug, serde::Serialize)]
pub(super) struct Finding {
    /// How much this matters.
    pub severity: Severity,
    /// What the finding is about: a component, an intervention, the model.
    pub subject: String,
    /// What was found.
    pub message: String,
    /// What to do about it.
    pub advice: String,
}

impl Finding {
    fn error(subject: impl Into<String>, message: impl Into<String>, advice: &str) -> Self {
        Self {
            severity: Severity::Error,
            subject: subject.into(),
            message: message.into(),
            advice: advice.to_owned(),
        }
    }

    fn warning(subject: impl Into<String>, message: impl Into<String>, advice: &str) -> Self {
        Self {
            severity: Severity::Warning,
            subject: subject.into(),
            message: message.into(),
            advice: advice.to_owned(),
        }
    }
}

/// Everything `check` found, in the shape a script reads it.
#[derive(Debug, serde::Serialize)]
pub(super) struct Diagnosis<'a> {
    name: &'a str,
    summary: &'a str,
    components: usize,
    relationships: usize,
    shared_quantities: usize,
    scale_units: usize,
    interventions: usize,
    /// Whether anything found would stop the design being solved.
    solvable: bool,
    findings: &'a [Finding],
}

impl<'a> Diagnosis<'a> {
    pub(super) fn new(loaded: &'a LoadedSystem, findings: &'a [Finding]) -> Self {
        Self {
            name: &loaded.name,
            summary: &loaded.summary,
            components: loaded.model.components.len(),
            relationships: loaded.model.relationships.len(),
            shared_quantities: loaded.model.scratchpad.len(),
            scale_units: loaded.model.scale_units.len(),
            interventions: loaded.model.interventions.len(),
            solvable: !fatal(findings),
            findings,
        }
    }
}

/// Everything `check` found, worst first.
///
/// Ordering is by severity and then by subject, so the thing that stops the
/// design solving is at the top of the list and repeated runs produce the same
/// report from the same design.
pub(super) fn findings(loaded: &LoadedSystem, solve: bool) -> Vec<Finding> {
    let mut findings = Vec::new();
    components(loaded, &mut findings);
    wiring(loaded, &mut findings);
    interventions(loaded, &mut findings);
    // A design already missing a required property will certainly fail to
    // solve, and reporting that as a second finding buries the first one.
    if solve && !fatal(&findings) {
        smoke_test(loaded, &mut findings);
    }
    findings.sort_by(|left, right| {
        left.severity
            .cmp(&right.severity)
            .then_with(|| left.subject.cmp(&right.subject))
    });
    findings
}

/// Reports whether any finding would stop the design being solved.
pub(super) fn fatal(findings: &[Finding]) -> bool {
    findings
        .iter()
        .any(|finding| finding.severity == Severity::Error)
}

fn components(loaded: &LoadedSystem, findings: &mut Vec<Finding>) {
    for component in &loaded.model.components {
        let Some(component_type) = loaded
            .component_types
            .get(component.component_type.as_str())
        else {
            findings.push(Finding::error(
                component.id.as_str(),
                format!("adopts the unknown type `{}`", component.component_type),
                "Run `optimist catalogue` to see the types this design can use, or add a \
                 manifest for this one under component-types/.",
            ));
            continue;
        };

        for name in component.properties.keys() {
            if !component_type.properties.contains_key(name) {
                findings.push(Finding::warning(
                    component.id.as_str(),
                    format!(
                        "sets `{name}`, which `{}` does not declare, so it is ignored",
                        component.component_type
                    ),
                    "Check the spelling against `optimist catalogue --type <ID>`.",
                ));
            }
        }

        for (name, property) in &component_type.properties {
            if property.is_required() && !component.properties.contains_key(name) {
                findings.push(Finding::error(
                    component.id.as_str(),
                    format!(
                        "does not supply `{name}`, which `{}` requires ({})",
                        component.component_type, property.unit
                    ),
                    "Give the property a value, or a default in the component type if every \
                     instance would use the same one.",
                ));
            }
        }
    }
}

fn wiring(loaded: &LoadedSystem, findings: &mut Vec<Finding>) {
    let wired: BTreeSet<_> = loaded
        .model
        .relationships
        .iter()
        .flat_map(|relationship| [&relationship.from, &relationship.to])
        .collect();
    for component in &loaded.model.components {
        if !wired.contains(&component.id) {
            findings.push(Finding::warning(
                component.id.as_str(),
                "is not wired to anything, so it neither offers nor receives load",
                "Add a relationship to or from it, or remove it from the design.",
            ));
        }
    }

    for relationship in &loaded.model.relationships {
        for attached in &relationship.mutators {
            if !loaded.mutators.contains_key(attached.mutator.as_str()) {
                findings.push(Finding::error(
                    format!("{} → {}", relationship.from, relationship.to),
                    format!("attaches the unknown behaviour `{}`", attached.mutator),
                    "Run `optimist catalogue` to see the behaviours this design can use.",
                ));
            }
        }
    }

    let known: BTreeSet<_> = loaded
        .model
        .components
        .iter()
        .map(|component| &component.id)
        .collect();
    for unit in &loaded.model.scale_units {
        for member in &unit.members {
            if !known.contains(member) {
                findings.push(Finding::error(
                    unit.id.as_str(),
                    format!("groups `{member}`, which is not a component of this design"),
                    "Correct the member list, or add the component it names.",
                ));
            }
        }
    }
}

fn interventions(loaded: &LoadedSystem, findings: &mut Vec<Finding>) {
    let shared: BTreeSet<&str> = loaded
        .model
        .scratchpad
        .iter()
        .map(|entry| entry.name.as_str())
        .collect();
    for intervention in &loaded.model.interventions {
        if intervention.overrides.is_empty() {
            findings.push(Finding::warning(
                intervention.id.as_str(),
                "rebinds nothing, so it compares identically against the design",
                "Name the quantity the change acts on and give its replacement value.",
            ));
        }
        for entry in &intervention.overrides {
            if !shared.contains(entry.name.as_str()) {
                findings.push(Finding::error(
                    intervention.id.as_str(),
                    format!("rebinds `{}`, which is not a shared quantity", entry.name),
                    "An intervention may only rebind scratchpad entries. Lift the quantity it \
                     acts on into the scratchpad first.",
                ));
            }
        }
    }
}

/// Solves the design once at low resolution to see whether it solves at all.
///
/// A handful of draws is enough: an expression that refers to a channel which
/// does not exist fails on the first one, and taking a thousand to discover
/// that wastes the time of somebody waiting on a check in continuous
/// integration.
fn smoke_test(loaded: &LoadedSystem, findings: &mut Vec<Finding>) {
    let config = EvaluationConfig {
        sample_count: 64,
        ..EvaluationConfig::default()
    };
    match Solve::new(&loaded.model, &loaded.component_types)
        .mutators(&loaded.mutators)
        .with(config)
        .evaluate()
    {
        Err(error) => findings.push(Finding::error(
            "model",
            format!("could not be solved: {error}"),
            "Fix the expression the message names, then run `optimist check` again.",
        )),
        Ok(evaluation) if !evaluation.settled().converged => findings.push(Finding::warning(
            "model",
            format!(
                "did not settle after {} passes; largest movement {}{}",
                evaluation.settled().iterations,
                super::render::number(evaluation.settled().movement),
                evaluation
                    .settled()
                    .unsettled
                    .as_ref()
                    .map(|moving| format!(" on `{}` of `{}`", moving.channel, moving.component))
                    .unwrap_or_default(),
            ),
            "A loop whose gain exceeds one has no steady state. Look for a component on a \
             response edge that publishes `rate`, or solve with `--transient` to watch it \
             diverge.",
        )),
        Ok(_) => {}
    }
}
