use std::path::PathBuf;

use anyhow::{Context as _, ensure};
use gpui::Context;

use super::RemoteFiles;
use crate::ssh::files::{Listing, RemotePath, Transfer, TransferItem};

fn items(paths: &[PathBuf], destination: &Listing) -> anyhow::Result<Vec<TransferItem>> {
    ensure!(!paths.is_empty(), "sftp: no files dropped");
    let mut names = std::collections::HashSet::new();
    paths
        .iter()
        .map(|path| {
            ensure!(path.is_absolute(), "sftp: dropped path must be absolute");
            let info = path
                .symlink_metadata()
                .with_context(|| format!("sftp: cannot read dropped file {}", path.display()))?;
            ensure!(
                info.is_file() || info.is_dir(),
                "sftp: drop a regular file or folder, not a symbolic link or special file"
            );
            let name = path
                .file_name()
                .context("sftp: dropped path has no filename")?;
            let name = RemotePath(name.as_encoded_bytes().to_vec());
            ensure!(
                names.insert(name.0.clone()),
                "sftp: dropped files have duplicate names"
            );
            if let Some(existing) = destination.entries.iter().find(|entry| entry.name == name) {
                ensure!(
                    !existing.link
                        && existing.directory == info.is_dir()
                        && (existing.directory || existing.regular),
                    "sftp: destination is a symbolic link, special file, or a different file type"
                );
            }
            Ok(TransferItem {
                local: path.clone(),
                remote: destination.path.join(&name.0)?,
                directory: info.is_dir(),
            })
        })
        .collect()
}

impl RemoteFiles {
    pub(super) fn drop_upload(&mut self, paths: &[PathBuf], cx: &mut Context<Self>) {
        let plan = || {
            ensure!(
                self.transfer.is_none() && self.pending.is_none(),
                "sftp: a transfer is already pending or running"
            );
            ensure!(!self.disconnected, "sftp: SSH session disconnected");
            ensure!(
                !self.remote.busy,
                "sftp: wait for the remote folder to finish reading"
            );
            let connection = self.connection.clone().context("sftp: no SSH connection")?;
            let destination = self
                .remote
                .listing
                .as_ref()
                .context("sftp: remote folder is unreadable")?;
            let transfer = Transfer {
                connection,
                upload: true,
                items: items(paths, destination)?,
            };
            transfer.batch()?;
            Ok::<_, anyhow::Error>(transfer)
        };
        self.transfer_error = None;
        match plan() {
            Ok(transfer) => {
                log::info!("sftp: confirming {} dropped items", transfer.items.len());
                self.pending = Some(transfer);
            }
            Err(error) => {
                log::error!("sftp: drop refused: {error:#}");
                self.transfer_error = Some(format!("{error:#}"));
            }
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dropped_paths_keep_the_destination_and_reject_invalid_sources() {
        let root = std::env::temp_dir().join(format!("mioterm-drop-{}", std::process::id()));
        std::fs::create_dir_all(root.join("folder")).unwrap();
        let file = root.join("file '[x].txt");
        std::fs::write(&file, b"fixture").unwrap();
        let mut listing = Listing {
            path: RemotePath(b"/destination".to_vec()),
            parent: RemotePath(b"/".to_vec()),
            entries: vec![],
        };
        let plan = items(&[file.clone(), root.join("folder")], &listing).unwrap();
        assert_eq!(plan[0].remote.0, b"/destination/file '[x].txt");
        assert!(!plan[0].directory);
        assert!(plan[1].directory);
        assert!(items(&[], &listing).is_err());
        assert!(items(&["relative".into()], &listing).is_err());
        assert!(items(&[root.join("missing")], &listing).is_err());
        assert!(items(&[file.clone(), file.clone()], &listing).is_err());
        listing.entries.push(crate::ssh::files::Entry {
            name: RemotePath(b"file '[x].txt".to_vec()),
            directory: true,
            regular: false,
            link: false,
            size: 0,
        });
        assert!(items(std::slice::from_ref(&file), &listing).is_err());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&file, root.join("link")).unwrap();
            assert!(items(&[root.join("link")], &listing).is_err());
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
