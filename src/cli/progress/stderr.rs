//! Standard error, described as a terminal whether or not it is one.
//!
//! `--progress always` exists for logs that are collected rather than watched,
//! which is where a solve that takes minutes is least visible and a line saying
//! it is still going is worth most. Nothing else in the tool needs to pretend
//! about a terminal, so this pretends as little as possible: the escapes it
//! writes are the four the drawing needs, and the width comes from `COLUMNS`
//! the same way the report layout reads it.

use std::io::{self, Write};

use indicatif::TermLike;

/// Width to lay a bar out in when nothing says how wide the output is.
const ASSUMED_WIDTH: u16 = 80;

#[derive(Debug)]
pub(super) struct Stderr;

impl Stderr {
    fn escape(&self, code: &str) -> io::Result<()> {
        self.write_str(&format!("\x1b[{code}"))
    }
}

impl TermLike for Stderr {
    fn width(&self) -> u16 {
        std::env::var("COLUMNS")
            .ok()
            .and_then(|columns| columns.parse().ok())
            .unwrap_or(ASSUMED_WIDTH)
    }

    fn move_cursor_up(&self, lines: usize) -> io::Result<()> {
        self.escape(&format!("{lines}A"))
    }

    fn move_cursor_down(&self, lines: usize) -> io::Result<()> {
        self.escape(&format!("{lines}B"))
    }

    fn move_cursor_right(&self, columns: usize) -> io::Result<()> {
        self.escape(&format!("{columns}C"))
    }

    fn move_cursor_left(&self, columns: usize) -> io::Result<()> {
        self.escape(&format!("{columns}D"))
    }

    fn write_line(&self, line: &str) -> io::Result<()> {
        self.write_str(line)?;
        self.write_str("\n")
    }

    fn write_str(&self, text: &str) -> io::Result<()> {
        io::stderr().lock().write_all(text.as_bytes())
    }

    fn clear_line(&self) -> io::Result<()> {
        self.escape("2K\r")
    }

    fn flush(&self) -> io::Result<()> {
        io::stderr().lock().flush()
    }
}
