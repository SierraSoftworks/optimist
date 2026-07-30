//! Confirming that a design moved between a directory and a file.

use crate::cli::render::{Report, Tone};

/// Says what was moved, where it went, and what to do with it next.
///
/// A transfer has no figures to lay out, and the only thing worth saying about
/// one is where the result is. What makes this worth rendering at all is the
/// sentence after that: somebody who has just exported a design wants to know
/// which file to send, and somebody who has just imported one wants to know
/// that checking it is the next thing to do.
pub(crate) fn transfer(action: &str, body: &str) -> Report {
    let mut report = Report::default();
    report.note(Tone::Good, action, body);
    report
}
