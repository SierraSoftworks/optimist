//! Packing a design directory into an archive.

use std::{
    fs,
    io::{Cursor, Write},
    path::Path,
};

use zip::{CompressionMethod, DateTime, ZipWriter, write::SimpleFileOptions};

use super::{ArchiveError, DIRECTORIES, MANIFEST};

/// Packs the design in `directory` into a zip archive.
///
/// Only the documents a design is made of are packed, so editor backups, notes,
/// and version control metadata sitting in the same directory stay where they
/// are. The entries are named under one folder, which is what a recipient
/// expects when they double-click the file.
///
/// Timestamps are fixed rather than taken from the filesystem, so packing an
/// unchanged design twice produces identical bytes and a checksum means
/// something.
///
/// ```no_run
/// use optimist::system::pack_system;
///
/// let archive = pack_system(std::path::Path::new("examples/checkout"))?;
/// std::fs::write("checkout.zip", archive)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn pack_system(directory: &Path) -> Result<Vec<u8>, ArchiveError> {
    let manifest = directory.join(MANIFEST);
    if !manifest.is_file() {
        return Err(ArchiveError::NotADesign);
    }

    let folder = folder_name(directory);
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    write(
        &mut writer,
        &format!("{folder}/{MANIFEST}"),
        &read(&manifest)?,
    )?;

    for section in DIRECTORIES {
        for path in documents(&directory.join(section))? {
            let Some(file) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let body = read(&path)?;
            write(&mut writer, &format!("{folder}/{section}/{file}"), &body)?;
        }
    }

    writer
        .finish()
        .map(Cursor::into_inner)
        .map_err(|error| ArchiveError::Unreadable {
            message: error.to_string(),
        })
}

/// Names the folder inside the archive after the design, where it can be.
///
/// A directory name that would not survive being unpacked somewhere else is
/// replaced rather than escaped, because the name is a convenience for whoever
/// opens the file and nothing reads it back.
fn folder_name(directory: &Path) -> String {
    directory
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| super::super::safe_identifier(name).ok())
        .unwrap_or("design")
        .to_owned()
}

/// Lists the YAML documents directly inside a directory, in a stable order.
fn documents(directory: &Path) -> Result<Vec<std::path::PathBuf>, ArchiveError> {
    if !directory.is_dir() {
        return Ok(Vec::new());
    }
    let entries = fs::read_dir(directory).map_err(|source| ArchiveError::Io {
        path: directory.display().to_string(),
        source,
    })?;
    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| ArchiveError::Io {
            path: directory.display().to_string(),
            source,
        })?;
        let path = entry.path();
        if path.is_file()
            && path
                .extension()
                .is_some_and(|extension| extension == "yaml")
        {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

fn read(path: &Path) -> Result<Vec<u8>, ArchiveError> {
    fs::read(path).map_err(|source| ArchiveError::Io {
        path: path.display().to_string(),
        source,
    })
}

fn write(
    writer: &mut ZipWriter<Cursor<Vec<u8>>>,
    name: &str,
    body: &[u8],
) -> Result<(), ArchiveError> {
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .last_modified_time(DateTime::default())
        .unix_permissions(0o644);
    writer
        .start_file(name, options)
        .and_then(|()| writer.write_all(body).map_err(Into::into))
        .map_err(|error| ArchiveError::Unreadable {
            message: error.to_string(),
        })
}
