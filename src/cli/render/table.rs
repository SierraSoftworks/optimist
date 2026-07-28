//! Columns that stay aligned once colour has been applied to them.
//!
//! A coloured string is longer than it looks: the escape sequences count
//! towards its byte and character length but occupy no space on screen. Every
//! cell therefore keeps its plain text and is painted only once the layout is
//! settled. Getting this the wrong way round is what makes coloured tables
//! drift out of alignment one row at a time.
//!
//! Tables are also given a width to fit inside. Where the natural columns do
//! not fit, the text ones give up space and the figures keep theirs: a name
//! ending in an ellipsis is still recognisable, and a number missing its last
//! digits is a different number.

use colored::Colorize;

use super::format::Tone;

/// A rendered line and the space it occupies on screen.
pub(super) struct Line {
    pub(super) text: String,
    pub(super) width: usize,
}

/// Which edge of its column a cell is pushed against.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Align {
    Left,
    Right,
}

/// One column of a table.
pub(crate) struct Column {
    title: String,
    align: Align,
}

impl Column {
    /// A column of names, read from the left and shortened when space is short.
    pub(crate) fn left(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            align: Align::Left,
        }
    }

    /// A column of figures, whose digits line up when read from the right.
    pub(crate) fn right(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            align: Align::Right,
        }
    }
}

/// One cell, kept as plain text until the layout around it is decided.
pub(crate) struct Cell {
    text: String,
    tone: Tone,
    painted: Option<String>,
}

impl Cell {
    /// A cell in the terminal's own foreground colour.
    pub(crate) fn plain(text: impl AsRef<str>) -> Self {
        Self::toned(text, Tone::Plain)
    }

    /// A cell painted with `tone`.
    pub(crate) fn toned(text: impl AsRef<str>, tone: Tone) -> Self {
        Self {
            text: text.as_ref().to_owned(),
            tone,
            painted: None,
        }
    }

    /// A cell assembled from differently coloured runs.
    ///
    /// This is how a before-and-after pair shares one column: each side keeps
    /// the colour that says how loaded it is, and the arrow between them stays
    /// out of the way.
    pub(crate) fn joined(parts: impl IntoIterator<Item = (String, Tone)>) -> Self {
        let parts: Vec<(String, Tone)> = parts.into_iter().collect();
        Self {
            text: parts.iter().map(|(text, _)| text.as_str()).collect(),
            tone: Tone::Plain,
            painted: Some(
                parts
                    .iter()
                    .map(|(text, tone)| tone.paint(text).to_string())
                    .collect(),
            ),
        }
    }

    /// A proportion drawn as a filled bar, so a column reads without arithmetic.
    ///
    /// Anything at or beyond its limit fills the bar completely; the figure
    /// beside it says by how much.
    pub(crate) fn bar(fraction: f64, width: usize) -> Self {
        let filled = if fraction.is_finite() {
            ((fraction.max(0.0) * width as f64).round() as usize).min(width)
        } else {
            width
        };
        Self {
            text: format!("{}{}", "█".repeat(filled), "░".repeat(width - filled)),
            tone: Tone::Plain,
            painted: Some(format!(
                "{}{}",
                Tone::for_utilisation(fraction).paint(&"█".repeat(filled)),
                Tone::Muted.paint(&"░".repeat(width - filled))
            )),
        }
    }

    fn width(&self) -> usize {
        self.text.chars().count()
    }

    fn render(&self, to: usize, align: Align) -> String {
        let text = elide(&self.text, to);
        let padding = " ".repeat(to.saturating_sub(text.chars().count()));
        let painted = match &self.painted {
            Some(painted) => painted.clone(),
            None => self.tone.paint(&text).to_string(),
        };
        match align {
            Align::Left => format!("{painted}{padding}"),
            Align::Right => format!("{padding}{painted}"),
        }
    }
}

fn elide(text: &str, to: usize) -> String {
    if text.chars().count() <= to {
        return text.to_owned();
    }
    text.chars()
        .take(to.saturating_sub(1))
        .chain(std::iter::once('…'))
        .collect()
}

/// A header and its rows, rendered as aligned columns.
pub(crate) struct Table {
    columns: Vec<Column>,
    rows: Vec<Vec<Cell>>,
}

/// The narrowest a text column is squeezed to before something else gives.
const FLOOR: usize = 6;
const GAP: &str = "  ";

impl Table {
    /// Starts a table with the given columns.
    pub(crate) fn new(columns: impl IntoIterator<Item = Column>) -> Self {
        Self {
            columns: columns.into_iter().collect(),
            rows: Vec::new(),
        }
    }

    /// Appends one row, padded or truncated to the column count.
    pub(crate) fn push(&mut self, cells: impl IntoIterator<Item = Cell>) -> &mut Self {
        let mut cells: Vec<Cell> = cells.into_iter().collect();
        cells.truncate(self.columns.len());
        while cells.len() < self.columns.len() {
            cells.push(Cell::plain(""));
        }
        self.rows.push(cells);
        self
    }

    /// Reports whether any rows have been added.
    pub(crate) fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub(super) fn lines(&self, budget: usize) -> Vec<Line> {
        let widths = self.widths(budget);
        let mut lines = vec![self.header(&widths), self.rule(&widths)];
        lines.extend(
            self.rows
                .iter()
                .map(|row| join(row, &widths, &self.columns)),
        );
        lines
    }

    /// Chooses a width per column, shrinking text columns until the row fits.
    fn widths(&self, budget: usize) -> Vec<usize> {
        let mut widths: Vec<usize> = self
            .columns
            .iter()
            .map(|column| column.title.chars().count())
            .collect();
        for row in &self.rows {
            for (index, cell) in row.iter().enumerate() {
                widths[index] = widths[index].max(cell.width());
            }
        }

        let gaps = GAP.len() * widths.len().saturating_sub(1);
        let mut total: usize = widths.iter().sum::<usize>() + gaps;
        while total > budget {
            let Some(widest) = self
                .columns
                .iter()
                .enumerate()
                .filter(|(index, column)| column.align == Align::Left && widths[*index] > FLOOR)
                .max_by_key(|(index, _)| widths[*index])
                .map(|(index, _)| index)
            else {
                break;
            };
            widths[widest] -= 1;
            total -= 1;
        }
        widths
    }

    fn header(&self, widths: &[usize]) -> Line {
        let cells: Vec<Cell> = self
            .columns
            .iter()
            .zip(widths)
            .map(|(column, width)| {
                let title = elide(&column.title.to_uppercase(), *width);
                Cell {
                    painted: Some(title.bold().to_string()),
                    text: title,
                    tone: Tone::Plain,
                }
            })
            .collect();
        join(&cells, widths, &self.columns)
    }

    fn rule(&self, widths: &[usize]) -> Line {
        let cells: Vec<Cell> = widths
            .iter()
            .map(|width| Cell::toned("─".repeat(*width), Tone::Muted))
            .collect();
        join(&cells, widths, &self.columns)
    }
}

fn join(cells: &[Cell], widths: &[usize], columns: &[Column]) -> Line {
    let text = cells
        .iter()
        .zip(widths)
        .zip(columns)
        .map(|((cell, width), column)| cell.render(*width, column.align))
        .collect::<Vec<_>>()
        .join(GAP);
    Line {
        text,
        width: widths.iter().sum::<usize>() + GAP.len() * widths.len().saturating_sub(1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Table {
        let mut table = Table::new([Column::left("component"), Column::right("utilisation")]);
        table.push([Cell::plain("orders"), Cell::plain("7.009")]);
        table.push([
            Cell::plain("a-considerably-longer-name"),
            Cell::plain("0.4"),
        ]);
        table
    }

    #[test]
    fn every_line_claims_the_same_width() {
        colored::control::set_override(true);
        let lines = sample().lines(200);
        let expected = "a-considerably-longer-name".len() + GAP.len() + "utilisation".len();
        assert!(lines.iter().all(|line| line.width == expected));
    }

    #[test]
    fn a_narrow_budget_shortens_names_and_spares_figures() {
        colored::control::set_override(false);
        let lines = sample().lines(24);
        assert!(lines.iter().all(|line| line.width <= 24));
        let rendered = lines
            .iter()
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains('…'), "{rendered}");
        assert!(rendered.contains("7.009"), "{rendered}");
    }

    #[test]
    fn a_bar_occupies_exactly_its_width_however_it_is_filled() {
        for fraction in [0.0, 0.5, 1.0, 12.0, f64::NAN] {
            assert_eq!(Cell::bar(fraction, 10).width(), 10);
        }
    }
}
