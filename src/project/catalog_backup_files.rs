use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use uuid::Uuid;

use super::catalog_backup::BackupError;

pub(super) fn write_immutable(path: &Path, bytes: &[u8]) -> Result<(), BackupError> {
    let parent = path.parent().expect("snapshot paths have a parent");
    fs::create_dir_all(parent).map_err(|source| io_error(parent.to_path_buf(), source))?;
    let temporary = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name().unwrap().to_string_lossy(),
        Uuid::new_v4()
    ));
    write_synced(&temporary, bytes, true)?;
    fs::hard_link(&temporary, path).map_err(|source| io_error(path.to_path_buf(), source))?;
    fs::remove_file(&temporary).map_err(|source| io_error(temporary, source))?;
    sync_directory(parent)
}

pub(super) fn write_synced(path: &Path, bytes: &[u8], create_new: bool) -> Result<(), BackupError> {
    let mut options = fs::OpenOptions::new();
    options
        .write(true)
        .create(true)
        .truncate(!create_new)
        .create_new(create_new);
    let mut file = options
        .open(path)
        .map_err(|source| io_error(path.to_path_buf(), source))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|source| io_error(path.to_path_buf(), source))
}

pub(super) fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, BackupError> {
    let bytes = fs::read(path).map_err(|source| io_error(path.to_path_buf(), source))?;
    serde_json::from_slice(&bytes).map_err(|source| BackupError::Json {
        path: path.to_path_buf(),
        source,
    })
}

pub(super) fn sync_directory(path: &Path) -> Result<(), BackupError> {
    fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|source| io_error(path.to_path_buf(), source))
}

pub(super) fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

pub(super) fn io_error(path: PathBuf, source: std::io::Error) -> BackupError {
    BackupError::Io { path, source }
}
