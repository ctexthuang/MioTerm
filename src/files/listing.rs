use std::{ffi::OsString, fs, io, path::Path, time::SystemTime};

/// A folder's entry: name, whether a folder, and modified time.
#[derive(Debug, PartialEq)]
pub(super) struct Entry {
    pub name: OsString,
    pub folder: bool,
    pub modified: SystemTime,
}

pub(super) type Listing = Result<Vec<Entry>, String>;

/// Reads folders before files, by name ignoring case.
pub(super) fn list(folder: &Path) -> Listing {
    let read = || -> io::Result<Vec<Entry>> {
        let mut entries = Vec::new();
        for entry in fs::read_dir(folder)? {
            let entry = entry?;
            let meta = match entry.metadata() {
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                meta => meta?,
            };
            let kind = meta.file_type();
            entries.push(Entry {
                folder: kind.is_dir() || (kind.is_symlink() && entry.path().is_dir()),
                name: entry.file_name(),
                modified: meta.modified()?,
            });
        }
        Ok(entries)
    };
    let mut entries = read().map_err(|error| error.to_string())?;
    entries.sort_by_cached_key(|entry| {
        let name = entry.name.to_string_lossy().to_lowercase();
        (!entry.folder, name, entry.name.clone())
    });
    Ok(entries)
}
