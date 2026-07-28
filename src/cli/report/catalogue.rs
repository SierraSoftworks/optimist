//! The component types and behaviours a design may draw on.

use crate::{
    cli::render::{Cell, Column, Report, Table, Tone},
    system::{ComponentType, LoadedSystem, Mutator},
};

/// Lists everything available to a design, shipped and project-local together.
pub(crate) fn catalogue(loaded: &LoadedSystem) -> Report {
    let mut types = Table::new([
        Column::left("type"),
        Column::left("name"),
        Column::right("properties"),
        Column::right("channels"),
        Column::right("limits"),
        Column::right("in use"),
    ]);
    for definition in loaded.component_types.values() {
        let used = loaded
            .model
            .components
            .iter()
            .filter(|component| component.component_type == definition.id)
            .count();
        types.push([
            Cell::toned(definition.id.as_str(), Tone::Name),
            Cell::toned(&definition.name, Tone::Muted),
            Cell::plain(definition.properties.len().to_string()),
            Cell::plain(definition.channels.len().to_string()),
            Cell::plain(definition.constraints.len().to_string()),
            Cell::toned(
                used.to_string(),
                if used > 0 { Tone::Good } else { Tone::Muted },
            ),
        ]);
    }

    let mut behaviours = Table::new([
        Column::left("behaviour"),
        Column::left("name"),
        Column::right("properties"),
        Column::right("requests"),
        Column::right("responses"),
    ]);
    for mutator in loaded.mutators.values() {
        behaviours.push([
            Cell::toned(mutator.id.as_str(), Tone::Name),
            Cell::toned(&mutator.name, Tone::Muted),
            Cell::plain(mutator.properties.len().to_string()),
            Cell::plain(mutator.requests.len().to_string()),
            Cell::plain(mutator.responses.len().to_string()),
        ]);
    }

    let mut report = Report::default();
    report.section("Component types", types);
    report.section("Behaviours", behaviours);
    report.note(
        Tone::Muted,
        "Reading one in full",
        "`optimist catalogue --type <ID>` prints the properties a type expects, the \
         quantities it derives, and the limits it can exhaust.",
    );
    report
}

/// Describes one component type or behaviour in full.
pub(crate) fn component_type(loaded: &LoadedSystem, id: &str) -> Option<Report> {
    if let Some(definition) = loaded.component_types.get(id) {
        return Some(from_type(definition));
    }
    loaded.mutators.get(id).map(from_mutator)
}

fn from_type(definition: &ComponentType) -> Report {
    let mut report = Report::default();
    report.note(Tone::Name, definition.id.as_str(), &definition.summary);

    let mut properties = Table::new([
        Column::left("property"),
        Column::left("unit"),
        Column::left("default"),
        Column::left("what it measures"),
    ]);
    for (name, property) in &definition.properties {
        properties.push([
            Cell::toned(name, Tone::Name),
            Cell::plain(&property.unit),
            match &property.default {
                Some(default) => Cell::toned(one_line(default), Tone::Muted),
                None => Cell::toned("required", Tone::Warn),
            },
            Cell::plain(one_line(&property.summary)),
        ]);
    }
    report.section("Properties", properties);

    let mut ports = Table::new([
        Column::left("port"),
        Column::left("direction"),
        Column::left("publishes"),
    ]);
    for (direction, side) in [
        ("in", &definition.ports.inbound),
        ("out", &definition.ports.outbound),
    ] {
        for (name, port) in side {
            ports.push([
                Cell::toned(name, Tone::Name),
                Cell::plain(direction),
                Cell::plain(names(port.publishes.keys())),
            ]);
        }
    }
    if !ports.is_empty() {
        report.section("Ports", ports);
    }

    let mut channels = Table::new([
        Column::left("channel"),
        Column::left("unit"),
        Column::left("expression"),
    ]);
    for (name, channel) in &definition.channels {
        channels.push([
            Cell::toned(name, Tone::Name),
            Cell::plain(&channel.unit),
            Cell::toned(one_line(&channel.expression), Tone::Muted),
        ]);
    }
    if !channels.is_empty() {
        report.section("Channels", channels);
    }

    let mut constraints = Table::new([
        Column::left("limit"),
        Column::left("demand"),
        Column::left("against"),
        Column::left("what saturating it does"),
    ]);
    for (name, constraint) in &definition.constraints {
        constraints.push([
            Cell::toned(name, Tone::Name),
            Cell::plain(&constraint.demand),
            Cell::plain(&constraint.limit),
            Cell::toned(one_line(&constraint.summary), Tone::Muted),
        ]);
    }
    if !constraints.is_empty() {
        report.section("Constraints", constraints);
    }

    report
}

fn from_mutator(mutator: &Mutator) -> Report {
    let mut report = Report::default();
    report.note(Tone::Name, mutator.id.as_str(), &mutator.summary);

    let mut properties = Table::new([
        Column::left("property"),
        Column::left("unit"),
        Column::left("default"),
        Column::left("what it measures"),
    ]);
    for (name, property) in &mutator.properties {
        properties.push([
            Cell::toned(name, Tone::Name),
            Cell::plain(&property.unit),
            match &property.default {
                Some(default) => Cell::toned(one_line(default), Tone::Muted),
                None => Cell::toned("required", Tone::Warn),
            },
            Cell::plain(one_line(&property.summary)),
        ]);
    }
    report.section("Properties", properties);

    let mut transforms = Table::new([
        Column::left("signal"),
        Column::left("direction"),
        Column::left("becomes"),
    ]);
    for (direction, side) in [
        ("request", &mutator.requests),
        ("response", &mutator.responses),
    ] {
        for (signal, transform) in side {
            transforms.push([
                Cell::toned(signal, Tone::Name),
                Cell::plain(direction),
                Cell::plain(one_line(&transform.expression)),
            ]);
        }
    }
    report.section("Rewrites", transforms);
    report
}

fn names<'a>(keys: impl Iterator<Item = &'a String>) -> String {
    let joined = keys.map(String::as_str).collect::<Vec<_>>().join(", ");
    if joined.is_empty() {
        "-".to_owned()
    } else {
        joined
    }
}

fn one_line(source: &str) -> String {
    source.split_whitespace().collect::<Vec<_>>().join(" ")
}
