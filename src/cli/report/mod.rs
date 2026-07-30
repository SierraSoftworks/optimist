//! Building the reports each command prints.
//!
//! Every command answers one question, and each of these modules turns the
//! answer to one of them into sections a reader can scan. Keeping that apart
//! from the commands themselves means the shape of a report is decided in one
//! place rather than being assembled inline wherever a command happens to
//! finish its work.

mod catalogue;
mod check;
mod limits;
mod solved;
mod transfer;

pub(super) use catalogue::{catalogue, component_type};
pub(super) use check::check;
pub(super) use limits::{bottlenecks, comparison};
pub(super) use solved::{channel_values, channels};
pub(super) use transfer::transfer;
