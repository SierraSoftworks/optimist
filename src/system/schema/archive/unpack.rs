//! Unpacking an archive from an untrusted source into a design directory.

use std::{
    fs,
    io::{Cursor, Read},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use zip::ZipArchive;

use super::{
    ArchiveError, DIRECTORIES, MANIFEST, MAX_ARCHIVE_BYTES, MAX_ENTRIES, MAX_ENTRY_BYTES,
    MAX_UNPACKED_BYTES,
};

/// Keeps concurrent imports in one process out of each other's scratch directory.
static SCRATCH: AtomicU64 = AtomicU64::new(0);

/// A design unpacked to a scratch directory and not yet in place.
///
/// Nothing an archive contains reaches the directory somebody is going to open
/// until the whole of it has been read, written out, and loaded by the ordinary
/// reader. A rejected archive therefore leaves the destination exactly as it
/// was, rather than half-replaced by a design that turned out to be unreadable.
///
/// Dropping this removes the scratch directory, so an import abandoned partway
/// through — including by an error on the way out — cleans up after itself.
///
/// ```no_run
/// use optimist::system::StagedDesign;
///
/// let archive = std::fs::read("checkout.zip")?;
/// let destination = std::path::Path::new("designs/checkout");
/// StagedDesign::stage(&archive, destination)?.install(destination)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct StagedDesign {
    path: PathBuf,
}

impl StagedDesign {
    /// Unpacks and validates `archive` beside where it is eventually going.
    ///
    /// The scratch directory is a sibling of `destination` so that putting the
    /// design in place is a rename rather than a copy, which is what makes the
    /// switch atomic enough that nobody reads a design mid-write.
    pub fn stage(archive: &[u8], destination: &Path) -> Result<Self, ArchiveError> {
        if archive.len() as u64 > MAX_ARCHIVE_BYTES {
            return Err(ArchiveError::TooLarge {
                limit: MAX_ARCHIVE_BYTES,
            });
        }
        let documents = read_documents(archive)?;

        let parent = destination.parent().unwrap_or(Path::new("."));
        create(parent)?;
        let staged = Self {
            path: parent.join(format!(
                ".optimist-import-{}-{}",
                std::process::id(),
                SCRATCH.fetch_add(1, Ordering::Relaxed)
            )),
        };
        remove(&staged.path)?;
        for (relative, body) in documents {
            let path = staged.path.join(&relative);
            if let Some(directory) = path.parent() {
                create(directory)?;
            }
            fs::write(&path, body).map_err(|source| ArchiveError::Io {
                path: path.display().to_string(),
                source,
            })?;
        }

        super::super::read_system(&staged.path)
            .map(|_| staged)
            .map_err(|source| ArchiveError::Invalid { source })
    }

    /// Borrows the scratch directory holding the validated design.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Moves the staged design onto `destination`, replacing anything there.
    pub fn install(self, destination: &Path) -> Result<(), ArchiveError> {
        remove(destination)?;
        fs::rename(&self.path, destination).map_err(|source| ArchiveError::Io {
            path: destination.display().to_string(),
            source,
        })
    }
}

impl Drop for StagedDesign {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// Reads every document a design is made of out of an archive, and nothing else.
///
/// Entry names are matched against the four paths a design has rather than being
/// sanitised, because sanitising invites an escape that the next encoding trick
/// gets past. A name that is not one of those paths never becomes a path at all.
fn read_documents(archive: &[u8]) -> Result<Vec<(String, Vec<u8>)>, ArchiveError> {
    let mut zip =
        ZipArchive::new(Cursor::new(archive)).map_err(|error| ArchiveError::Unreadable {
            message: error.to_string(),
        })?;
    if zip.len() > MAX_ENTRIES {
        return Err(ArchiveError::TooManyEntries { limit: MAX_ENTRIES });
    }

    let names = (0..zip.len())
        .map(|index| zip.name_for_index(index).unwrap_or_default().to_owned())
        .collect::<Vec<_>>();
    let prefix = manifest_prefix(&names).ok_or(ArchiveError::NotADesign)?;

    let mut documents = Vec::new();
    let mut unpacked = 0u64;
    for (index, name) in names.iter().enumerate() {
        let Some(relative) = wanted(name, &prefix)? else {
            continue;
        };
        let mut entry = zip
            .by_index(index)
            .map_err(|error| ArchiveError::Unreadable {
                message: error.to_string(),
            })?;
        // A symbolic link is stored as a file whose contents are its target, so
        // extracting one would write a path where a document should be.
        if entry.unix_mode().is_some_and(is_symlink) {
            return Err(ArchiveError::Misplaced {
                entry: name.clone(),
            });
        }

        let mut body = Vec::new();
        entry
            .by_ref()
            .take(MAX_ENTRY_BYTES + 1)
            .read_to_end(&mut body)
            .map_err(|error| ArchiveError::Unreadable {
                message: error.to_string(),
            })?;
        if body.len() as u64 > MAX_ENTRY_BYTES {
            return Err(ArchiveError::TooLarge {
                limit: MAX_ENTRY_BYTES,
            });
        }
        unpacked += body.len() as u64;
        if unpacked > MAX_UNPACKED_BYTES {
            return Err(ArchiveError::TooLarge {
                limit: MAX_UNPACKED_BYTES,
            });
        }
        documents.push((relative, body));
    }
    Ok(documents)
}

fn is_symlink(mode: u32) -> bool {
    mode & 0o170_000 == 0o120_000
}

/// Returns where the design sits inside the archive, as a prefix to strip.
///
/// Somebody who zips a design directory produces `checkout/_system.yaml`, and
/// somebody who zips its contents produces `_system.yaml`. Both are what they
/// meant, so the shallowest manifest decides which one this is.
fn manifest_prefix(names: &[String]) -> Option<String> {
    names
        .iter()
        .map(|name| normalise(name))
        .filter(|name| !is_noise(name))
        .filter_map(|name| name.strip_suffix(MANIFEST).map(str::to_owned))
        .filter(|prefix| prefix.is_empty() || prefix.ends_with('/'))
        .min_by_key(|prefix| prefix.matches('/').count())
}

/// Returns the path an entry should be written to, where it is part of a design.
///
/// A stray YAML document is refused rather than ignored: it is far more likely
/// to be a component somebody moved than something they meant to leave behind,
/// and quietly importing a design missing a component is the worst outcome
/// available. Everything else — readmes, images, the litter archivers add — is
/// simply not extracted.
fn wanted(name: &str, prefix: &str) -> Result<Option<String>, ArchiveError> {
    let name = normalise(name);
    if is_noise(&name) || name.ends_with('/') {
        return Ok(None);
    }
    let Some(relative) = name.strip_prefix(prefix) else {
        return refuse(&name);
    };

    if relative == MANIFEST {
        return Ok(Some(MANIFEST.to_owned()));
    }
    if let Some((section, file)) = relative.split_once('/')
        && DIRECTORIES.contains(&section)
        && let Some(stem) = file.strip_suffix(".yaml")
        && super::super::safe_identifier(stem).is_ok()
    {
        return Ok(Some(format!("{section}/{file}")));
    }
    refuse(&name)
}

fn refuse(name: &str) -> Result<Option<String>, ArchiveError> {
    if name.ends_with(".yaml") || name.ends_with(".yml") {
        return Err(ArchiveError::Misplaced {
            entry: name.to_owned(),
        });
    }
    Ok(None)
}

/// Reads an entry name the way every archiver on every platform writes one.
fn normalise(name: &str) -> String {
    name.replace('\\', "/").trim_start_matches("./").to_owned()
}

/// Reports whether an entry is something an archiver added rather than a person.
fn is_noise(name: &str) -> bool {
    name.starts_with("__MACOSX/")
        || name.starts_with(".git/")
        || name
            .rsplit('/')
            .next()
            .is_some_and(|file| file == ".DS_Store" || file == "Thumbs.db")
}

fn create(directory: &Path) -> Result<(), ArchiveError> {
    fs::create_dir_all(directory).map_err(|source| ArchiveError::Io {
        path: directory.display().to_string(),
        source,
    })
}

fn remove(directory: &Path) -> Result<(), ArchiveError> {
    match fs::remove_dir_all(directory) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(ArchiveError::Io {
            path: directory.display().to_string(),
            source,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_design_at_the_archive_root_needs_no_prefix() {
        let names = vec!["_system.yaml".to_owned(), "components/api.yaml".to_owned()];
        assert_eq!(manifest_prefix(&names), Some(String::new()));
    }

    #[test]
    fn a_design_inside_one_folder_is_found_through_it() {
        let names = vec![
            "checkout/".to_owned(),
            "checkout/_system.yaml".to_owned(),
            "checkout/components/api.yaml".to_owned(),
        ];
        assert_eq!(manifest_prefix(&names), Some("checkout/".to_owned()));
    }

    #[test]
    fn an_archive_without_a_manifest_is_not_a_design() {
        assert_eq!(manifest_prefix(&["components/api.yaml".to_owned()]), None);
    }

    #[test]
    fn entry_names_cannot_escape_the_directory_they_unpack_into() {
        for name in [
            "../_system.yaml",
            "components/../../escape.yaml",
            "/etc/passwd.yaml",
            "components\\..\\..\\escape.yaml",
            "C:/windows/system.yaml",
        ] {
            assert!(
                wanted(name, "").is_err(),
                "'{name}' should not be extracted"
            );
        }
    }

    #[test]
    fn only_the_documents_a_design_is_made_of_are_extracted() {
        assert_eq!(
            wanted("_system.yaml", "").unwrap().as_deref(),
            Some("_system.yaml")
        );
        assert_eq!(
            wanted("d/components/api.yaml", "d/").unwrap().as_deref(),
            Some("components/api.yaml")
        );
        assert_eq!(
            wanted("mutators/retry.yaml", "").unwrap().as_deref(),
            Some("mutators/retry.yaml")
        );
        assert_eq!(wanted("README.md", "").unwrap(), None);
        assert_eq!(wanted("__MACOSX/_system.yaml", "").unwrap(), None);
        assert_eq!(wanted(".DS_Store", "").unwrap(), None);
    }

    #[test]
    fn a_yaml_document_somewhere_a_design_has_no_place_for_is_refused() {
        for name in [
            "notes.yaml",
            "components/nested/api.yaml",
            "components/API.yaml",
        ] {
            assert!(wanted(name, "").is_err(), "'{name}' should be refused");
        }
    }
}
