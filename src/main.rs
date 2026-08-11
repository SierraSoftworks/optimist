// A window build asks Windows for no console of its own, or one would open
// behind the application. The same binary still runs the command line, so it
// reattaches to whichever console launched it instead.
#![cfg_attr(all(feature = "desktop", windows), windows_subsystem = "windows")]

use std::process::ExitCode;

use clap::Parser;
use optimist::cli::{Cli, run};

// A solve allocates a sample set per derived quantity and drops it a pass later,
// tens of thousands of times a second and on every share at once. Measured on
// the shipped examples this is worth about a seventh undivided and better than a
// third once the draws are divided, because the system allocator serialises what
// the shares are doing in parallel.
#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() -> ExitCode {
    attach_console();
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{}", human_errors::pretty(&err));
            ExitCode::FAILURE
        }
    }
}

/// Writes to the console this was launched from, if there was one.
///
/// A window build is compiled without a console, so a report printed by
/// `optimist check` would otherwise go nowhere when it is run from a terminal.
#[cfg(all(feature = "desktop", windows))]
fn attach_console() {
    use windows_sys::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};

    // SAFETY: the call takes no pointers and fails harmlessly when there is no
    // parent console to attach to, which is the case for a double-clicked app.
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

#[cfg(not(all(feature = "desktop", windows)))]
fn attach_console() {}
