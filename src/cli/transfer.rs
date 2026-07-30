//! Sharing a design as a file, and taking one that was shared.
//!
//! A design is a directory, and a directory is not something anybody attaches to
//! a review or sends to a colleague. These two commands are the whole of moving
//! one between machines: pack it, send the file, unpack it.
//!
//! Importing treats the file as hostile, because by the time it reaches this
//! command it has been through at least one system nobody here controls. It is
//! unpacked and loaded in full before the destination is touched, so a file that
//! turns out not to be a design leaves whatever was already there alone.

use std::path::{Path, PathBuf};

use clap::Args;

use crate::system::{ArchiveError, StagedDesign, pack_system, read_system};

use super::{output::OutputFormat, system::print};

#[derive(Debug, Args)]
pub(super) struct ExportArgs {
    /// Directory holding the design.
    #[arg(default_value = ".")]
    directory: PathBuf,
    /// Where to write the archive; `-` writes it to standard output.
    archive: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub(super) struct ImportArgs {
    /// The archive to unpack.
    archive: PathBuf,
    /// Directory to unpack it into; defaults to the archive's own name.
    directory: Option<PathBuf>,
    /// Replace the design already in that directory.
    #[arg(long)]
    force: bool,
}

/// Packs a design into a file somebody else can open.
pub(super) fn export(args: ExportArgs, output: OutputFormat) -> Result<(), human_errors::Error> {
    let archive = pack_system(&args.directory).map_err(|error| refused(&args.directory, error))?;
    let destination = args
        .archive
        .unwrap_or_else(|| PathBuf::from(format!("{}.zip", stem(&args.directory))));

    if destination == Path::new("-") {
        return to_stdout(&archive);
    }
    std::fs::write(&destination, &archive).map_err(|error| {
        human_errors::user(
            format!("{} could not be written: {error}", destination.display()),
            &["Check that the directory it names exists and can be written to."],
        )
    })?;

    print(output.transfer(
        "Exported",
        &destination,
        &format!(
            "{} is packed into {}, {} bytes. Send that file to share the design.",
            args.directory.display(),
            destination.display(),
            archive.len()
        ),
    )?)
}

/// Unpacks a shared archive into a design directory.
pub(super) fn import(args: ImportArgs, output: OutputFormat) -> Result<(), human_errors::Error> {
    let archive = std::fs::read(&args.archive).map_err(|error| {
        human_errors::user(
            format!("{} could not be read: {error}", args.archive.display()),
            &["Check the path, and that the file finished downloading."],
        )
    })?;
    let destination = args
        .directory
        .unwrap_or_else(|| PathBuf::from(stem(&args.archive)));

    if !args.force && destination.join("_system.yaml").is_file() {
        return Err(human_errors::user(
            format!("There is already a design in {}.", destination.display()),
            &[
                "Name a directory to unpack into, as `optimist import <archive> <directory>`.",
                "Pass --force to replace the design that is there, losing whatever it holds.",
            ],
        ));
    }

    let staged = StagedDesign::stage(&archive, &destination)
        .map_err(|error| refused(&args.archive, error))?;
    let components = read_system(staged.path())
        .map(|loaded| loaded.model.components.len())
        .unwrap_or_default();
    staged
        .install(&destination)
        .map_err(|error| refused(&args.archive, error))?;

    print(output.transfer(
        "Imported",
        &destination,
        &format!(
            "{} is unpacked into {}, holding {} components. Run `optimist check {}` to look it over.",
            args.archive.display(),
            destination.display(),
            components,
            destination.display()
        ),
    )?)
}

/// Names a directory or archive the way its own file is named.
fn stem(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("design")
        .to_owned()
}

/// Writes an archive to standard output, for piping somewhere else.
fn to_stdout(archive: &[u8]) -> Result<(), human_errors::Error> {
    use std::io::Write;

    let mut stdout = std::io::stdout().lock();
    match stdout.write_all(archive).and_then(|()| stdout.flush()) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
        Err(error) => Err(human_errors::system(
            format!("The archive could not be written: {error}"),
            &["Redirect the output to a file, as `optimist export . - > design.zip`."],
        )),
    }
}

/// Reports an archive failure with the guidance the archive itself supplies.
fn refused(path: &Path, error: ArchiveError) -> human_errors::Error {
    human_errors::user(format!("{}: {error}", path.display()), error.advice())
}
