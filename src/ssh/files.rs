use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::Duration,
};

use anyhow::{Context as _, ensure};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::remote::Remote;

pub mod panel;
#[cfg(test)]
mod tests;
mod transfer;

pub use transfer::{Transfer, TransferItem};

const SCRIPT: &str = include_str!("../../assets/ssh/files.py");

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemotePath(pub Vec<u8>);

impl Serialize for RemotePath {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&STANDARD.encode(&self.0))
    }
}

impl<'de> Deserialize<'de> for RemotePath {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        let bytes = STANDARD.decode(text).map_err(serde::de::Error::custom)?;
        if bytes.is_empty() || bytes.contains(&0) {
            return Err(serde::de::Error::custom("remote files: empty path or NUL"));
        }
        Ok(Self(bytes))
    }
}

impl RemotePath {
    pub fn shown(&self) -> String {
        crate::sessions::one_line(&String::from_utf8_lossy(&self.0))
    }

    pub fn join(&self, name: &[u8]) -> anyhow::Result<Self> {
        ensure!(
            !name.is_empty()
                && name != b"."
                && name != b".."
                && !name.contains(&b'/')
                && !name.contains(&0),
            "remote files: invalid entry name"
        );
        let mut path = self.0.clone();
        if !path.ends_with(b"/") {
            path.push(b'/');
        }
        path.extend_from_slice(name);
        Ok(Self(path))
    }

    pub fn parent(&self) -> anyhow::Result<Self> {
        let at = self
            .0
            .iter()
            .rposition(|byte| *byte == b'/')
            .context("remote files: path has no parent")?;
        Ok(Self(self.0[..at.max(1)].to_vec()))
    }

    pub fn local_path(&self) -> anyhow::Result<PathBuf> {
        #[cfg(unix)]
        return Ok(PathBuf::from(
            <OsStr as std::os::unix::ffi::OsStrExt>::from_bytes(&self.0),
        ));
        #[cfg(not(unix))]
        Ok(PathBuf::from(
            std::str::from_utf8(&self.0).context("remote files: path is not UTF-8")?,
        ))
    }

    pub fn from_path(path: &Path) -> Self {
        Self(path.as_os_str().as_encoded_bytes().to_vec())
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub name: RemotePath,
    pub directory: bool,
    pub regular: bool,
    pub link: bool,
    pub size: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Listing {
    pub path: RemotePath,
    pub parent: RemotePath,
    pub entries: Vec<Entry>,
}

impl Remote {
    pub fn list(&self, path: Option<&RemotePath>, cancel: &AtomicBool) -> anyhow::Result<Listing> {
        let request = serde_json::to_vec(&serde_json::json!({"path": path}))?;
        let args = ["python3", "-c", SCRIPT].map(OsStr::new);
        let out = self.output(&args, &request, cancel, Duration::from_secs(20))?;
        ensure!(
            out.status.success(),
            "remote files: cannot list {}: {}",
            self.host.destination(),
            String::from_utf8_lossy(&out.stderr).trim()
        );
        let mut listing: Listing = serde_json::from_slice(&out.stdout)
            .context("remote files: invalid directory response (server requires Python 3)")?;
        ensure!(
            listing.path.0.starts_with(b"/") && listing.parent.0.starts_with(b"/"),
            "remote files: server returned a relative folder"
        );
        ensure!(
            listing.entries.len() <= 10000,
            "remote files: directory exceeds 10000 entries"
        );
        let mut names = std::collections::HashSet::new();
        for entry in &listing.entries {
            listing.path.join(&entry.name.0)?;
            ensure!(
                names.insert(&entry.name.0),
                "remote files: duplicate entry name"
            );
        }
        listing.entries.sort_by_cached_key(|entry| {
            (
                !entry.directory,
                String::from_utf8_lossy(&entry.name.0).to_lowercase(),
                entry.name.0.clone(),
            )
        });
        Ok(listing)
    }
}
