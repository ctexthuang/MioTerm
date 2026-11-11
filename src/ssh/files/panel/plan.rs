use anyhow::ensure;
use gpui::SharedString;

use crate::ssh::files::{Entry, Listing, Remote, RemotePath, Transfer, TransferItem};

pub(super) fn key(
    session: Option<u64>,
    local: bool,
    folder: Option<&RemotePath>,
    entry: &Entry,
) -> SharedString {
    serde_json::to_string(&(session, local, folder, &entry.name))
        .expect("file identifiers serialize")
        .into()
}

pub(super) fn selected(keys: &[SharedString], clicked: &SharedString) -> Vec<SharedString> {
    if keys.contains(clicked) {
        keys.to_vec()
    } else {
        vec![clicked.clone()]
    }
}

pub(super) fn terminal_path(
    session: u64,
    listing: &Listing,
    dragged: &SharedString,
) -> Option<std::path::PathBuf> {
    let entry = listing
        .entries
        .iter()
        .find(|entry| key(Some(session), false, Some(&listing.path), entry) == *dragged)?;
    listing.path.join(&entry.name.0).ok()?.local_path().ok()
}

pub(super) fn build(
    connection: &Remote,
    local: &Listing,
    remote: &Listing,
    upload: bool,
    selected: &[SharedString],
) -> anyhow::Result<Transfer> {
    ensure!(!selected.is_empty(), "sftp: no file selected");
    let (source, destination) = if upload {
        (local, remote)
    } else {
        (remote, local)
    };
    let mut items = Vec::new();
    for entry in &source.entries {
        if !selected.contains(&key(
            Some(connection.session),
            upload,
            Some(&source.path),
            entry,
        )) {
            continue;
        }
        ensure!(
            !entry.link && (entry.regular || entry.directory),
            "sftp: select a regular file or folder, not a symbolic link or special file"
        );
        if let Some(existing) = destination
            .entries
            .iter()
            .find(|item| item.name == entry.name)
        {
            ensure!(
                !existing.link
                    && existing.directory == entry.directory
                    && (existing.directory || existing.regular),
                "sftp: destination is a symbolic link, special file, or a different file type"
            );
        }
        items.push(TransferItem {
            local: local
                .path
                .local_path()?
                .join(super::actions::filename(&entry.name)?),
            remote: remote.path.join(&entry.name.0)?,
            directory: entry.directory,
        });
    }
    ensure!(
        items.len() == selected.len(),
        "sftp: selected files or source folder changed"
    );
    let transfer = Transfer {
        connection: connection.clone(),
        upload,
        items,
    };
    transfer.batch()?;
    Ok(transfer)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (Remote, Listing, Listing) {
        let connection = Remote {
            host: crate::ssh::Host::default(),
            socket: None,
            session: 42,
        };
        let listing = |path: &std::path::Path| Listing {
            path: RemotePath::from_path(path),
            parent: RemotePath::from_path(path),
            entries: ["first.txt", "folder"]
                .into_iter()
                .enumerate()
                .map(|(index, name)| Entry {
                    name: RemotePath(name.as_bytes().to_vec()),
                    directory: index == 1,
                    regular: index == 0,
                    link: false,
                    size: 0,
                })
                .collect(),
        };
        let local = listing(&std::env::temp_dir());
        let remote = listing(std::path::Path::new("/remote"));
        (connection, local, remote)
    }

    #[test]
    fn file_keys_bind_the_pane_session_and_original_folder() {
        let (mut connection, local, remote) = fixture();
        let keys: Vec<_> = local
            .entries
            .iter()
            .map(|entry| key(Some(42), true, Some(&local.path), entry))
            .collect();
        let transfer = build(&connection, &local, &remote, true, &keys).unwrap();
        assert!(transfer.upload);
        assert_eq!(transfer.items.len(), 2);
        assert_eq!(transfer.items[0].remote.0, b"/remote/first.txt");
        assert!(transfer.items[1].directory);
        assert!(build(&connection, &local, &remote, false, &keys).is_err());
        connection.session += 1;
        assert!(build(&connection, &local, &remote, true, &keys).is_err());
        let changed = Listing {
            path: RemotePath(b"/other".to_vec()),
            ..local
        };
        connection.session = 42;
        assert!(build(&connection, &changed, &remote, true, &keys).is_err());
    }

    #[test]
    fn remote_downloads_keep_the_local_destination_and_validate_every_selection() {
        let (connection, local, mut remote) = fixture();
        let keys: Vec<_> = remote
            .entries
            .iter()
            .map(|entry| key(Some(42), false, Some(&remote.path), entry))
            .collect();
        let transfer = build(&connection, &local, &remote, false, &keys).unwrap();
        assert!(!transfer.upload);
        assert_eq!(
            transfer.items[0].local,
            std::env::temp_dir().join("first.txt")
        );
        remote.entries.pop();
        assert!(build(&connection, &local, &remote, false, &keys).is_err());
        remote.entries[0].link = true;
        assert!(build(&connection, &local, &remote, false, &keys[..1]).is_err());
    }

    #[test]
    fn terminal_drags_only_resolve_the_current_remote_pane() {
        let (_, local, remote) = fixture();
        let dragged = key(Some(42), false, Some(&remote.path), &remote.entries[0]);
        assert_eq!(
            terminal_path(42, &remote, &dragged)
                .unwrap()
                .to_string_lossy(),
            "/remote/first.txt"
        );
        assert!(terminal_path(43, &remote, &dragged).is_none());
        assert!(terminal_path(42, &local, &dragged).is_none());
        let local_drag = key(Some(42), true, Some(&local.path), &local.entries[0]);
        assert!(terminal_path(42, &remote, &local_drag).is_none());
    }

    #[test]
    fn right_click_and_drag_keep_a_selected_group_or_pick_the_new_row() {
        let selected = vec![SharedString::from("first"), SharedString::from("second")];
        assert_eq!(super::selected(&selected, &selected[1]), selected);
        assert_eq!(
            super::selected(&selected, &"third".into()),
            vec![SharedString::from("third")]
        );
    }
}
