//! What a design is closest to exhausting, and what a change does about it.

use crate::{
    cli::render::{Cell, Column, Report, Table, Tone, number, percentage, plural, ratio},
    system::{Bottleneck, Comparison, Movement},
};

const BAR: usize = 12;

/// Ranks the constraints a design is closest to exhausting, worst first.
pub(crate) fn bottlenecks(ranked: &[Bottleneck]) -> Report {
    let mut report = Report::default();
    if ranked.is_empty() {
        report.note(
            Tone::Good,
            "Nothing is binding",
            "No constraint in this design comes close to its limit in any draw.",
        );
        return report;
    }

    // Replicas are worth a column only where they differ; a design with none
    // would otherwise carry a column of ones down every report it produces.
    let replicated = ranked.iter().any(|entry| entry.replicas != 1.0);
    let mut columns = vec![
        Column::left("component"),
        Column::left("constraint"),
        Column::left("load"),
        Column::right("mean"),
        Column::right("p90"),
        Column::right("binds"),
        Column::right("headroom"),
    ];
    if replicated {
        columns.push(Column::right("replicas"));
    }

    let mut table = Table::new(columns);
    for entry in ranked {
        let mut cells = vec![
            Cell::toned(entry.component.as_str(), Tone::Name),
            Cell::plain(&entry.constraint),
            Cell::bar(entry.utilisation, BAR),
            Cell::toned(
                ratio(entry.utilisation),
                Tone::for_utilisation(entry.utilisation),
            ),
            Cell::toned(
                ratio(entry.utilisation_p90),
                Tone::for_utilisation(entry.utilisation_p90),
            ),
            Cell::toned(
                percentage(entry.probability_of_binding),
                Tone::for_probability(entry.probability_of_binding),
            ),
            Cell::toned(
                number(entry.headroom),
                if entry.headroom < 0.0 {
                    Tone::Bad
                } else {
                    Tone::Muted
                },
            ),
        ];
        if replicated {
            cells.push(Cell::toned(number(entry.replicas), Tone::Muted));
        }
        table.push(cells);
    }
    report.section("Constraints", table);

    let worst = &ranked[0];
    if worst.binds() {
        report.note(
            Tone::Bad,
            format!("{}.{} runs out first", worst.component, worst.constraint),
            format!(
                "It is carrying {}× what its limit allows on average and binds in {} of draws. \
                 {}",
                ratio(worst.utilisation),
                percentage(worst.probability_of_binding),
                worst.summary
            ),
        );
    } else {
        report.note(
            Tone::Good,
            "Nothing binds",
            "Every constraint stays within its limit in every draw, so the design has \
             headroom everywhere it was asked about.",
        );
    }
    report
}

/// Weighs one or more proposals against the design they would replace.
pub(crate) fn comparison(named: &[(String, Comparison)]) -> Report {
    let mut report = Report::default();
    for (intervention, comparison) in named {
        let mut table = Table::new([
            Column::left("component"),
            Column::left("constraint"),
            Column::right("utilisation"),
            Column::right("binds"),
            Column::left("effect"),
        ]);
        for movement in &comparison.movements {
            let (effect, tone) = effect(movement);
            table.push([
                Cell::toned(movement.component.as_str(), Tone::Name),
                Cell::plain(&movement.constraint),
                Cell::joined([
                    (
                        ratio(movement.before),
                        Tone::for_utilisation(movement.before),
                    ),
                    (" → ".to_owned(), Tone::Muted),
                    (ratio(movement.after), Tone::for_utilisation(movement.after)),
                ]),
                Cell::joined([
                    (
                        percentage(movement.bound_before),
                        Tone::for_probability(movement.bound_before),
                    ),
                    (" → ".to_owned(), Tone::Muted),
                    (
                        percentage(movement.bound_after),
                        Tone::for_probability(movement.bound_after),
                    ),
                ]),
                Cell::toned(effect, tone),
            ]);
        }
        report.section(intervention, table);
        verdict(&mut report, intervention, comparison);
    }
    report
}

fn verdict(report: &mut Report, intervention: &str, comparison: &Comparison) {
    let relieved = comparison.relieved();
    let introduced = comparison.introduced();
    let persisting: Vec<&Movement> = comparison
        .movements
        .iter()
        .filter(|movement| movement.bound_before > 0.0 && movement.bound_after > 0.0)
        .collect();

    let still = if persisting.is_empty() {
        String::new()
    } else {
        format!(
            " {} still binding afterwards: {}.",
            plural(persisting.len(), "constraint is", "constraints are"),
            subjects(&persisting)
        )
    };

    if !introduced.is_empty() {
        // Relieving one limit routinely promotes another, and a change that
        // only moves the bottleneck is worth recognising before it is built.
        report.note(
            Tone::Warn,
            format!("{intervention} moves the bottleneck"),
            format!(
                "It starts {} binding that did not before: {}. Relieving one limit routinely \
                 promotes another, so decide whether this is a fix or a move before building \
                 it.{still}",
                plural(introduced.len(), "constraint", "constraints"),
                subjects(&introduced),
            ),
        );
    } else if !relieved.is_empty() {
        report.note(
            if persisting.is_empty() {
                Tone::Good
            } else {
                Tone::Warn
            },
            format!("{intervention} relieves what it was aimed at"),
            format!(
                "It stops {} binding and starts none: {}.{still}",
                plural(relieved.len(), "constraint", "constraints"),
                subjects(&relieved),
            ),
        );
    } else {
        report.note(
            Tone::Muted,
            format!("{intervention} changes nothing that binds"),
            format!(
                "No constraint started or stopped binding under this change, so whatever it \
                 moved was not what the design is short of.{still}"
            ),
        );
    }
}

fn effect(movement: &Movement) -> (&'static str, Tone) {
    if movement.relieved() {
        ("relieved", Tone::Good)
    } else if movement.introduced() {
        ("introduced", Tone::Bad)
    } else if movement.shift() < 0.0 {
        ("eased", Tone::Good)
    } else if movement.shift() > 0.0 {
        ("loaded", Tone::Warn)
    } else {
        ("unchanged", Tone::Muted)
    }
}

fn subjects(movements: &[&Movement]) -> String {
    movements
        .iter()
        .map(|movement| format!("{}.{}", movement.component, movement.constraint))
        .collect::<Vec<_>>()
        .join(", ")
}
