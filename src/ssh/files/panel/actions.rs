use std::path::Component;

use anyhow::{Context as _, ensure};
use gpui::{AppContext, Context, SharedString};

use super::{Cancel, RemoteFiles};
use crate::ssh::files::{RemotePath, Transfer, TransferItem};

fn filename(name: &RemotePath) -> anyhow::Result<std::path::PathBuf> {
    let path = name.local_path()?;
    ensure!(
        matches!(
            path.components().collect::<Vec<_>>().as_slice(),
            [Component::Normal(_)]
        ),
        "sftp: remote filename is not a local filename"
    );
    #[cfg(windows)]
    {
        let text = path.to_str().context("sftp: filename is not UTF-8")?;
        let stem = text.split('.').next().unwrap_or("").to_ascii_uppercase();
        ensure!(
            !text
                .chars()
                .any(|c| "<>:\"/\\|?*".contains(c) || c.is_control())
                && !text.ends_with(['.', ' '])
                && ![
                    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6",
                    "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7",
                    "LPT8", "LPT9"
                ]
                .contains(&stem.as_str()),
            "sftp: filename cannot be represented on Windows"
        );
    }
    Ok(path)
}

impl RemoteFiles {
    pub(super) fn activate(&mut self, local: bool, key: &SharedString, cx: &mut Context<Self>) {
        let Some(listing) = &self.browser(local).listing else {
            return;
        };
        let Some(entry) = listing
            .entries
            .iter()
            .find(|entry| super::view::key(entry) == *key)
        else {
            return;
        };
        if entry.directory {
            let path = if local {
                listing
                    .path
                    .local_path()
                    .and_then(|path| Ok(RemotePath::from_path(&path.join(filename(&entry.name)?))))
            } else {
                listing.path.join(&entry.name.0)
            };
            match path {
                Ok(path) => self.navigate(local, Some(path), cx),
                Err(error) => self.browser_mut(local).error = Some(format!("{error:#}")),
            }
        } else if local {
            if let Ok(path) = listing
                .path
                .local_path()
                .and_then(|path| Ok(path.join(filename(&entry.name)?)))
            {
                cx.open_with_system(&path);
            }
        } else {
            self.remote.selected = vec![key.clone()];
            self.ask(false, cx);
        }
        cx.notify();
    }

    fn plan(&self, upload: bool) -> anyhow::Result<Transfer> {
        ensure!(!self.disconnected, "sftp: SSH session disconnected");
        ensure!(
            !self.local.busy && !self.remote.busy,
            "sftp: wait for folders to finish reading"
        );
        let connection = self.connection.clone().context("sftp: no SSH connection")?;
        let local = self
            .local
            .listing
            .as_ref()
            .context("sftp: local folder is unreadable")?;
        let remote = self
            .remote
            .listing
            .as_ref()
            .context("sftp: remote folder is unreadable")?;
        let browser = self.browser(upload);
        let source = if upload { local } else { remote };
        ensure!(!browser.selected.is_empty(), "sftp: no file selected");
        let mut items = Vec::new();
        for entry in source
            .entries
            .iter()
            .filter(|entry| browser.selected.contains(&super::view::key(entry)))
        {
            ensure!(
                !entry.link && (entry.regular || entry.directory),
                "sftp: select a regular file or folder, not a symbolic link or special file"
            );
            let local = local.path.local_path()?.join(filename(&entry.name)?);
            let remote = remote.path.join(&entry.name.0)?;
            let destination = if upload {
                &self.remote.listing
            } else {
                &self.local.listing
            };
            if let Some(destination) = destination
                .as_ref()
                .and_then(|listing| listing.entries.iter().find(|item| item.name == entry.name))
            {
                ensure!(
                    !destination.link
                        && destination.directory == entry.directory
                        && (destination.directory || destination.regular),
                    "sftp: destination is a symbolic link, special file, or a different file type"
                );
            }
            items.push(TransferItem {
                local,
                remote,
                directory: entry.directory,
            });
        }
        ensure!(
            !items.is_empty(),
            "sftp: selected files are no longer listed"
        );
        let transfer = Transfer {
            connection,
            upload,
            items,
        };
        transfer.batch()?;
        Ok(transfer)
    }

    pub(super) fn ask(&mut self, upload: bool, cx: &mut Context<Self>) {
        if self.transfer.is_some() {
            return;
        }
        self.transfer_error = None;
        match self.plan(upload) {
            Ok(transfer) => self.pending = Some(transfer),
            Err(error) => self.transfer_error = Some(format!("{error:#}")),
        }
        cx.notify();
    }

    pub(super) fn transfer(&mut self, cx: &mut Context<Self>) {
        let Some(transfer) = self.pending.take() else {
            return;
        };
        if self.connection.as_ref() != Some(&transfer.connection) || self.disconnected {
            self.transfer_error = Some("sftp: connection changed; transfer cancelled.".into());
            return cx.notify();
        }
        let guard = Cancel::default();
        let cancel = guard.0.clone();
        let connection = transfer.connection.clone();
        self.transfer = Some(guard);
        self.transfer_status = Some(format!(
            "{} {} item(s) on {}...",
            if transfer.upload {
                "Uploading"
            } else {
                "Downloading"
            },
            transfer.items.len(),
            connection.host.label()
        ));
        let task = cx.background_spawn(async move { transfer.run(&cancel) });
        self.transferring = Some(cx.spawn(async move |panel, cx| {
            let result = task.await;
            panel
                .update(cx, |panel, cx| {
                    panel.transfer = None;
                    match result {
                        Ok(()) => {
                            panel.transfer_status =
                                Some(format!("Transfer complete: {}", connection.host.label()))
                        }
                        Err(error) => {
                            log::error!("sftp: {error:#}");
                            panel.transfer_status = None;
                            panel.transfer_error =
                                Some(format!("{}: {error:#}", connection.host.label()));
                        }
                    }
                    panel.refresh_local(cx);
                    if panel.connection.as_ref() == Some(&connection) && !panel.disconnected {
                        panel.refresh_remote(cx);
                    }
                    cx.notify();
                })
                .ok();
        }));
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_names_cannot_escape_the_local_destination() {
        for name in [b"..".as_slice(), b".", b"/tmp/a", b"a/b"] {
            assert!(filename(&RemotePath(name.to_vec())).is_err());
        }
        assert_eq!(
            filename(&RemotePath(b"normal.txt".to_vec())).unwrap(),
            std::path::Path::new("normal.txt")
        );
    }
}
