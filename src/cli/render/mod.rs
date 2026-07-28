//! Reports rendered for a terminal.
//!
//! The presentation follows the one this project already uses for errors: a
//! rounded box per section, a coloured title, and content padded to a common
//! width. Somebody who has seen an `optimist` failure recognises the shape of
//! an `optimist` report, and the two stack on top of each other without
//! looking like output from two different programs.
//!
//! Colour is decoration, never information. Every tone is an emphasis on a
//! figure that is also written out, so a report piped into a file, read by a
//! screen reader, or captured by an agent loses nothing. [`colored`] suppresses
//! escape sequences when the stream is not a terminal, which makes that the
//! default rather than something a caller has to remember.

mod format;
mod table;

use std::fmt::{Display, Formatter, Result};

use colored::Colorize;

pub(crate) use format::{Tone, number, percentage, plural, quantity, ratio};
pub(crate) use table::{Cell, Column, Table};

use table::Line;

/// A sequence of boxed sections, written out in the order they were added.
#[derive(Default)]
pub(crate) struct Report {
    blocks: Vec<Block>,
}

enum Block {
    Section {
        title: String,
        table: Table,
    },
    Note {
        tone: Tone,
        title: String,
        body: String,
    },
}

impl Report {
    /// Appends a titled table.
    pub(crate) fn section(&mut self, title: impl Into<String>, table: Table) -> &mut Self {
        self.blocks.push(Block::Section {
            title: title.into(),
            table,
        });
        self
    }

    /// Appends a titled passage of prose, wrapped to the width of the report.
    ///
    /// This is where a report says what its numbers mean: that a model never
    /// settled, that a change moved the bottleneck rather than removing it, or
    /// that nothing is wrong. A table alone leaves the reader to work that out.
    pub(crate) fn note(
        &mut self,
        tone: Tone,
        title: impl Into<String>,
        body: impl Into<String>,
    ) -> &mut Self {
        self.blocks.push(Block::Note {
            tone,
            title: title.into(),
            body: body.into(),
        });
        self
    }

    /// Reports whether anything has been added.
    pub(crate) fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }
}

impl Display for Report {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result {
        let inner = available_width() - 4;
        let blocks: Vec<(Tone, &str, Vec<Line>)> = self
            .blocks
            .iter()
            .map(|block| match block {
                Block::Section { title, table } => (Tone::Name, title.as_str(), table.lines(inner)),
                Block::Note { tone, title, body } => {
                    (*tone, title.as_str(), wrapped(body, *tone, inner))
                }
            })
            .collect();

        // Every box in a report shares one width, so a reader's eye follows a
        // single pair of edges down the page rather than a ragged one per
        // section, and no box is ever wider than the terminal reading it.
        let width = blocks
            .iter()
            .map(|(_, title, lines)| {
                lines
                    .iter()
                    .map(|line| line.width)
                    .max()
                    .unwrap_or(0)
                    .max(title.chars().count() + 2)
                    + 4
            })
            .max()
            .unwrap_or(0)
            .min(inner + 4);

        for (index, (tone, title, lines)) in blocks.iter().enumerate() {
            if index > 0 {
                writeln!(formatter)?;
            }
            write_box(formatter, title, *tone, lines, width)?;
        }
        Ok(())
    }
}

/// The width a report is laid out against.
///
/// `COLUMNS` wins over the terminal's own answer so that a report captured by a
/// script, a test, or an agent can be pinned to a known width instead of
/// depending on whatever device happens to be attached to the process.
fn available_width() -> usize {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or_else(textwrap::termwidth)
        .clamp(48, 200)
}

fn wrapped(body: &str, tone: Tone, width: usize) -> Vec<Line> {
    body.lines()
        .flat_map(|paragraph| {
            // A bullet's continuation is indented past its marker; ordinary
            // prose is not, because a hanging indent on a paragraph reads as a
            // mistake rather than as structure.
            let indent = if paragraph.starts_with(" • ") {
                "   "
            } else {
                ""
            };
            textwrap::wrap(
                paragraph,
                textwrap::Options::new(width).subsequent_indent(indent),
            )
            .into_iter()
            .map(|chunk| Line {
                width: chunk.chars().count(),
                text: tone.paint(&chunk).to_string(),
            })
            .collect::<Vec<_>>()
        })
        .collect()
}

const HORIZONTAL: &str = "─";

fn write_box(
    formatter: &mut Formatter<'_>,
    title: &str,
    tone: Tone,
    lines: &[Line],
    width: usize,
) -> Result {
    writeln!(
        formatter,
        "{} {} {}{}",
        "╭─".bright_black(),
        tone.paint(title).bold(),
        HORIZONTAL
            .repeat(width.saturating_sub(title.chars().count() + 5))
            .bright_black(),
        "╮".bright_black()
    )?;
    for line in lines {
        writeln!(
            formatter,
            "{} {}{} {}",
            "│".bright_black(),
            line.text,
            " ".repeat((width - 4).saturating_sub(line.width)),
            "│".bright_black()
        )?;
    }
    writeln!(
        formatter,
        "{}{}{}",
        "╰".bright_black(),
        HORIZONTAL.repeat(width - 2).bright_black(),
        "╯".bright_black()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(report: &Report) -> String {
        colored::control::set_override(false);
        report.to_string()
    }

    #[test]
    fn a_section_is_boxed_to_the_width_of_its_widest_line() {
        let mut table = Table::new([Column::left("component"), Column::right("utilisation")]);
        table.push([Cell::plain("orders"), Cell::plain("7.009")]);

        let mut report = Report::default();
        report.section("Bottlenecks", table);

        let rendered = plain(&report);
        let widths: Vec<usize> = rendered.lines().map(|line| line.chars().count()).collect();
        assert!(
            widths.windows(2).all(|pair| pair[0] == pair[1]),
            "{rendered}"
        );
        assert!(rendered.contains("COMPONENT"));
        assert!(rendered.contains("orders"));
    }

    #[test]
    fn a_note_wraps_rather_than_overflowing() {
        let mut report = Report::default();
        report.note(
            Tone::Warn,
            "Did not settle",
            "A loop whose gain exceeds one has no steady state to find, so there is nothing \
             for the solver to converge on however many passes it is given.",
        );

        let rendered = plain(&report);
        assert!(rendered.lines().all(|line| line.chars().count() <= 100));
        assert!(rendered.contains("Did not settle"));
    }
}
