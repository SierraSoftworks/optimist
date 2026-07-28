//! What a design contains, and what is wrong with it.

use crate::{
    cli::{
        diagnose::{Finding, Severity},
        render::{Cell, Column, Report, Table, Tone, plural},
    },
    system::LoadedSystem,
};

/// Describes a design and reports everything `check` found in it.
pub(crate) fn check(loaded: &LoadedSystem, findings: &[Finding]) -> Report {
    let mut report = Report::default();
    report.note(Tone::Name, &loaded.name, summary(loaded));
    report.section("Components", components(loaded));

    if !loaded.model.scratchpad.is_empty() {
        report.section("Shared quantities", scratchpad(loaded));
    }
    if !loaded.model.interventions.is_empty() {
        report.section("Interventions", interventions(loaded));
    }

    problems(&mut report, findings);
    report
}

fn summary(loaded: &LoadedSystem) -> String {
    let counts = format!(
        "{}, {}, {}, {}, {}.",
        plural(loaded.model.components.len(), "component", "components"),
        plural(
            loaded.model.relationships.len(),
            "relationship",
            "relationships"
        ),
        plural(
            loaded.model.scratchpad.len(),
            "shared quantity",
            "shared quantities"
        ),
        plural(loaded.model.scale_units.len(), "scale unit", "scale units"),
        plural(
            loaded.model.interventions.len(),
            "intervention",
            "interventions"
        ),
    );
    if loaded.summary.is_empty() {
        counts
    } else {
        format!("{}\n{counts}", loaded.summary)
    }
}

fn components(loaded: &LoadedSystem) -> Table {
    let mut table = Table::new([
        Column::left("id"),
        Column::left("type"),
        Column::left("name"),
        Column::right("calls"),
        Column::right("callers"),
        Column::right("behaviours"),
    ]);
    for component in &loaded.model.components {
        let outbound = loaded.model.outbound_from(&component.id);
        let behaviours: usize = outbound
            .iter()
            .map(|relationship| relationship.mutators.len())
            .sum();
        table.push([
            Cell::toned(component.id.as_str(), Tone::Name),
            Cell::plain(component.component_type.as_str()),
            Cell::toned(&component.name, Tone::Muted),
            Cell::plain(outbound.len().to_string()),
            Cell::plain(loaded.model.inbound_to(&component.id).len().to_string()),
            Cell::plain(behaviours.to_string()),
        ]);
    }
    table
}

fn scratchpad(loaded: &LoadedSystem) -> Table {
    let mut table = Table::new([
        Column::left("name"),
        Column::left("unit"),
        Column::left("expression"),
    ]);
    for entry in &loaded.model.scratchpad {
        table.push([
            Cell::toned(&entry.name, Tone::Name),
            Cell::toned(entry.unit.as_deref().unwrap_or("-"), Tone::Muted),
            Cell::plain(one_line(&entry.expression)),
        ]);
    }
    table
}

fn interventions(loaded: &LoadedSystem) -> Table {
    let mut table = Table::new([
        Column::left("id"),
        Column::left("name"),
        Column::left("rebinds"),
    ]);
    for intervention in &loaded.model.interventions {
        let rebound = intervention
            .overrides
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        table.push([
            Cell::toned(intervention.id.as_str(), Tone::Name),
            Cell::toned(&intervention.name, Tone::Muted),
            Cell::plain(if rebound.is_empty() {
                "nothing".to_owned()
            } else {
                rebound
            }),
        ]);
    }
    table
}

fn problems(report: &mut Report, findings: &[Finding]) {
    if findings.is_empty() {
        report.note(
            Tone::Good,
            "Nothing to report",
            "The design loads, every component supplies what its type needs, and it solves.",
        );
        return;
    }

    let mut table = Table::new([
        Column::left("severity"),
        Column::left("subject"),
        Column::left("finding"),
    ]);
    for finding in findings {
        let tone = match finding.severity {
            Severity::Error => Tone::Bad,
            Severity::Warning => Tone::Warn,
        };
        table.push([
            Cell::toned(label(finding.severity), tone),
            Cell::toned(&finding.subject, Tone::Name),
            Cell::plain(&finding.message),
        ]);
    }
    report.section("Findings", table);

    let mut seen = std::collections::BTreeSet::new();
    let advice: Vec<&str> = findings
        .iter()
        .map(|finding| finding.advice.as_str())
        .filter(|advice| seen.insert(*advice))
        .collect();
    let errors = findings
        .iter()
        .filter(|finding| finding.severity == Severity::Error)
        .count();
    let (tone, title) = if errors > 0 {
        (
            Tone::Bad,
            format!("{} to fix", plural(errors, "problem", "problems")),
        )
    } else {
        (Tone::Warn, "Worth a look".to_owned())
    };
    report.note(tone, title, format!(" • {}", advice.join("\n • ")));
}

fn label(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
    }
}

fn one_line(source: &str) -> String {
    source.split_whitespace().collect::<Vec<_>>().join(" ")
}
