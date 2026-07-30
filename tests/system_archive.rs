//! Coverage for moving a design in and out of an archive.
//!
//! The round trip is worth pinning because a design that comes back different
//! from how it went is worse than one that would not travel at all. Everything
//! after it is about archives nobody here wrote: the point of these is that a
//! file which is not a design leaves the destination exactly as it was, and says
//! why in terms the person holding it can act on.

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use optimist::system::{ArchiveError, StagedDesign, pack_system, read_system};
use rstest::rstest;
use zip::{ZipWriter, write::SimpleFileOptions};

/// A throwaway directory that removes itself.
struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "optimist-archive-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("scratch directory");
        Self { path }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn example(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join(name)
}

/// Builds an archive from entry names and bodies, exactly as given.
fn archive(entries: &[(&str, &str)]) -> Vec<u8> {
    let mut writer = ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, body) in entries {
        writer
            .start_file(*name, SimpleFileOptions::default())
            .expect("entry starts");
        writer.write_all(body.as_bytes()).expect("entry is written");
    }
    writer.finish().expect("archive closes").into_inner()
}

/// The smallest thing that loads as a design.
fn manifest() -> &'static str {
    "schema_version: 2\nname: Shared\nsummary: A design.\n"
}

fn install(bytes: &[u8], destination: &Path) -> Result<(), ArchiveError> {
    StagedDesign::stage(bytes, destination)?.install(destination)
}

/// A design that goes out and comes back is the same design.
#[test]
fn a_design_survives_the_round_trip() {
    let scratch = Scratch::new("round-trip");
    let source = example("checkout");
    let packed = pack_system(&source).expect("packs");

    let destination = scratch.path.join("checkout");
    install(&packed, &destination).expect("installs");

    let before = read_system(&source).expect("reads the original");
    let after = read_system(&destination).expect("reads the copy");
    assert_eq!(after.name, before.name);
    assert_eq!(after.summary, before.summary);
    assert_eq!(
        format!("{:?}", after.model),
        format!("{:?}", before.model),
        "the model changed in transit"
    );
}

/// Packing the same design twice produces the same bytes.
///
/// Without this, an archive committed beside a proposal shows up as changed
/// every time somebody regenerates it, and a checksum says nothing.
#[test]
fn packing_is_reproducible() {
    let source = example("checkout");
    assert_eq!(
        pack_system(&source).expect("packs"),
        pack_system(&source).expect("packs again")
    );
}

/// Project-local definitions travel with the design that needs them.
#[test]
fn locally_defined_types_and_behaviours_travel_too() {
    let scratch = Scratch::new("local-definitions");
    let source = example("deadlines");
    assert!(source.join("mutators").is_dir(), "the example moved");

    let destination = scratch.path.join("deadlines");
    install(&pack_system(&source).expect("packs"), &destination).expect("installs");

    let mutators = fs::read_dir(source.join("mutators"))
        .expect("reads")
        .count();
    assert_eq!(
        fs::read_dir(destination.join("mutators"))
            .expect("reads")
            .count(),
        mutators
    );
}

/// A design zipped from inside its own directory imports just as well.
#[rstest]
#[case::inside_a_folder("checkout/")]
#[case::at_the_root("")]
fn a_design_is_found_however_the_archive_was_made(#[case] prefix: &str) {
    let scratch = Scratch::new("prefix");
    let bytes = archive(&[
        (&format!("{prefix}_system.yaml"), manifest()),
        (
            &format!("{prefix}components/api.yaml"),
            "id: api\nname: API\ntype: compute\nproperties:\n  service_time: '0.01'\n  parallelism: '8'\n",
        ),
    ]);

    let destination = scratch.path.join("design");
    install(&bytes, &destination).expect("installs");
    assert_eq!(
        read_system(&destination)
            .expect("reads")
            .model
            .components
            .len(),
        1
    );
}

/// Nothing an archive says can put a file outside the directory it unpacks into.
#[rstest]
#[case::parent("../escaped.yaml")]
#[case::deep_parent("components/../../escaped.yaml")]
#[case::absolute("/etc/optimist.yaml")]
#[case::windows_absolute("C:/windows/optimist.yaml")]
#[case::backslash_parent("components\\..\\..\\escaped.yaml")]
fn an_entry_cannot_escape_the_directory_it_unpacks_into(#[case] entry: &str) {
    let scratch = Scratch::new("escape");
    let bytes = archive(&[("_system.yaml", manifest()), (entry, "id: escaped\n")]);

    let destination = scratch.path.join("design");
    assert!(
        matches!(
            install(&bytes, &destination),
            Err(ArchiveError::Misplaced { .. })
        ),
        "'{entry}' was not refused"
    );
    assert!(!scratch.path.join("escaped.yaml").exists());
    assert!(!destination.exists(), "a refused archive left files behind");
}

/// Files that are not part of a design are simply not extracted.
#[test]
fn incidental_files_are_left_out_rather_than_refused() {
    let scratch = Scratch::new("incidental");
    let bytes = archive(&[
        ("_system.yaml", manifest()),
        ("README.md", "notes"),
        (".DS_Store", "junk"),
        ("__MACOSX/._system.yaml", "junk"),
    ]);

    let destination = scratch.path.join("design");
    install(&bytes, &destination).expect("installs");
    assert!(!destination.join("README.md").exists());
    assert!(!destination.join(".DS_Store").exists());
}

/// An archive that is not a design says so rather than producing an empty one.
#[test]
fn an_archive_without_a_manifest_is_refused() {
    let scratch = Scratch::new("no-manifest");
    let bytes = archive(&[("notes/todo.txt", "nothing here")]);
    assert!(matches!(
        install(&bytes, &scratch.path.join("design")),
        Err(ArchiveError::NotADesign)
    ));
}

/// A file that is not an archive at all is refused before anything is written.
#[test]
fn a_file_that_is_not_an_archive_is_refused() {
    let scratch = Scratch::new("not-an-archive");
    let error =
        install(b"this is not a zip file", &scratch.path.join("design")).expect_err("is refused");
    assert!(matches!(error, ArchiveError::Unreadable { .. }));
    assert!(!error.advice().is_empty(), "a refusal offers nothing to do");
}

/// A design written by a schema this build does not read is named as such.
#[test]
fn an_archive_from_an_unsupported_version_is_refused_with_the_version() {
    let scratch = Scratch::new("version");
    let bytes = archive(&[(
        "_system.yaml",
        "schema_version: 99\nname: Future\nsummary: ''\n",
    )]);

    let error = install(&bytes, &scratch.path.join("design")).expect_err("is refused");
    assert!(
        error.to_string().contains("99"),
        "the version it was written by is not named: {error}"
    );
    assert!(matches!(error, ArchiveError::Invalid { .. }));
}

/// An archive that expands far beyond any design is stopped rather than unpacked.
#[test]
fn an_archive_that_expands_beyond_reason_is_refused() {
    let scratch = Scratch::new("bomb");
    let mut writer = ZipWriter::new(std::io::Cursor::new(Vec::new()));
    writer
        .start_file("_system.yaml", SimpleFileOptions::default())
        .expect("entry starts");
    writer.write_all(manifest().as_bytes()).expect("writes");
    writer
        .start_file("components/api.yaml", SimpleFileOptions::default())
        .expect("entry starts");
    // Highly compressible, so the archive stays small while the entry does not.
    writer.write_all(&vec![b'a'; 8 << 20]).expect("writes");
    let bytes = writer.finish().expect("closes").into_inner();

    assert!(matches!(
        install(&bytes, &scratch.path.join("design")),
        Err(ArchiveError::TooLarge { .. })
    ));
}

/// An archive that will not load leaves the design it was replacing untouched.
#[test]
fn a_rejected_archive_does_not_disturb_what_is_already_there() {
    let scratch = Scratch::new("untouched");
    let destination = scratch.path.join("checkout");
    install(
        &pack_system(&example("checkout")).expect("packs"),
        &destination,
    )
    .expect("installs");

    let broken = archive(&[
        ("_system.yaml", manifest()),
        ("components/api.yaml", "id: api\ntype: no-such-type\n"),
    ]);
    assert!(StagedDesign::stage(&broken, &destination).is_err());

    assert_eq!(
        read_system(&destination).expect("still reads").name,
        "Checkout"
    );
}

/// Staging leaves nothing behind when the design it produced is never installed.
#[test]
fn an_abandoned_import_cleans_up_after_itself() {
    let scratch = Scratch::new("abandoned");
    let destination = scratch.path.join("design");
    let bytes = archive(&[("_system.yaml", manifest())]);

    drop(StagedDesign::stage(&bytes, &destination).expect("stages"));

    let leftovers = fs::read_dir(&scratch.path).expect("reads").count();
    assert_eq!(leftovers, 0, "a scratch directory was left behind");
}
