//! Source-located Squiggle diagnostics rendered by Ariadne.

use std::io;

use ariadne::{Color, Config, IndexType, Label, Report, ReportKind, Source};

use super::ast::Span;

/// The stage that produced a Squiggle diagnostic.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticKind {
    /// Source could not be tokenized or parsed.
    Syntax,
    /// Evaluation failed after parsing.
    Runtime,
    /// Static analysis found an issue before evaluation.
    Lint,
}

/// An actionable error tied to a byte range in the authored source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    /// Error category.
    pub kind: DiagnosticKind,
    /// Concise error description.
    pub message: String,
    /// Relevant source range.
    pub span: Span,
    /// Optional recovery guidance.
    pub help: Option<String>,
}

impl Diagnostic {
    /// Creates a syntax diagnostic.
    pub(crate) fn syntax(message: impl Into<String>, span: Span) -> Self {
        Self {
            kind: DiagnosticKind::Syntax,
            message: message.into(),
            span,
            help: None,
        }
    }

    /// Creates a runtime diagnostic.
    pub(crate) fn runtime(message: impl Into<String>, span: Span) -> Self {
        Self {
            kind: DiagnosticKind::Runtime,
            message: message.into(),
            span,
            help: None,
        }
    }

    /// Creates a static-analysis diagnostic.
    pub(crate) fn lint(message: impl Into<String>, span: Span) -> Self {
        Self {
            kind: DiagnosticKind::Lint,
            message: message.into(),
            span,
            help: None,
        }
    }

    /// Adds recovery guidance to this diagnostic.
    #[must_use]
    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    /// Renders the diagnostic and source excerpt as plain ANSI-free UTF-8 text.
    pub fn render(&self, source: &str) -> Result<String, String> {
        let mut output = Vec::new();
        self.write(source, &mut output)
            .map_err(|error| format!("failed to render diagnostic: {error}"))?;
        String::from_utf8(output)
            .map_err(|error| format!("diagnostic renderer produced invalid UTF-8: {error}"))
    }

    /// Writes an Ariadne report with a source excerpt to `writer`.
    pub fn write(&self, source: &str, writer: impl io::Write) -> io::Result<()> {
        let start = self.span.start.min(source.len());
        let end = self.span.end.max(start.saturating_add(1)).min(source.len());
        let range = start..end;
        let mut report = Report::build(ReportKind::Error, ((), range.clone()))
            .with_config(
                Config::new()
                    .with_color(false)
                    .with_index_type(IndexType::Byte),
            )
            .with_message(&self.message)
            .with_label(
                Label::new(((), range))
                    .with_message(&self.message)
                    .with_color(Color::Red),
            );
        if let Some(help) = &self.help {
            report = report.with_help(help);
        }
        report.finish().write(Source::from(source), writer)
    }
}
