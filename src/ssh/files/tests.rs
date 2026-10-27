use super::*;

#[test]
fn remote_paths_keep_bytes_and_posix_separators() {
    let path = RemotePath(b"/tmp/a\\b".to_vec()).join(b"raw\xff").unwrap();
    assert_eq!(path.0, b"/tmp/a\\b/raw\xff");
    assert_eq!(
        serde_json::from_slice::<RemotePath>(&serde_json::to_vec(&path).unwrap()).unwrap(),
        path
    );
    for name in [b"..".as_slice(), b".", b"", b"a/b", b"a\0b"] {
        assert!(path.join(name).is_err());
    }
}

#[test]
fn sftp_batch_quotes_literals_and_refuses_command_injection() {
    let connection = Remote {
        session: 0,
        host: super::super::Host {
            address: "example.test".into(),
            ..Default::default()
        },
        socket: None,
    };
    let mut transfer = Transfer {
        connection,
        upload: false,
        items: vec![TransferItem {
            local: std::env::temp_dir().join("a\"b"),
            remote: RemotePath(b"/root/a*[b] \"c\\".to_vec()),
            directory: false,
        }],
    };
    let batch = String::from_utf8(transfer.batch().unwrap()).unwrap();
    assert!(batch.starts_with("@get \"/root/a*[b] \\\"c\\\\\" "));
    assert!(batch.ends_with("\nbye\n"));
    transfer.items[0].remote = RemotePath(b"/root/a\n!touch /tmp/not-allowed".to_vec());
    assert!(transfer.batch().is_err());
}

#[cfg(target_os = "macos")]
#[test]
fn real_sftp_round_trips_literal_names_and_recursive_folders() {
    use std::{fs, process::Command};
    let root = std::env::temp_dir().join(format!(
        "ely-sftp-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    let source = root.join("source");
    let destination = root.join("remote");
    let download = root.join("download");
    for path in [&source, &destination, &download] {
        fs::create_dir(path).unwrap();
    }
    let name = "literal *[x]? '\"\\中.txt";
    fs::write(source.join(name), b"literal payload").unwrap();
    fs::create_dir(source.join("folder space")).unwrap();
    fs::write(source.join("folder space/nested.txt"), b"nested").unwrap();
    let connection = Remote {
        session: 0,
        host: super::super::Host {
            address: "fixture".into(),
            ..Default::default()
        },
        socket: None,
    };
    let mut upload = Transfer {
        connection: connection.clone(),
        upload: true,
        items: vec![
            TransferItem {
                local: source.join(name),
                remote: RemotePath::from_path(&destination.join(name)),
                directory: false,
            },
            TransferItem {
                local: source.join("folder space"),
                remote: RemotePath::from_path(&destination.join("folder space")),
                directory: true,
            },
        ],
    };
    let run = |transfer: &Transfer| {
        let mut command = Command::new("/usr/bin/sftp");
        command.args(["-D", "/usr/libexec/sftp-server", "-b", "-", "fixture"]);
        let out = crate::ssh::remote::process::output(
            &mut command,
            &transfer.batch().unwrap(),
            &AtomicBool::new(false),
            Duration::from_secs(10),
        )
        .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    };
    run(&upload);
    assert_eq!(
        fs::read(destination.join(name)).unwrap(),
        b"literal payload"
    );
    assert_eq!(
        fs::read(destination.join("folder space/nested.txt")).unwrap(),
        b"nested"
    );
    // Uploading an existing folder must merge it, not nest another copy.
    fs::write(source.join("folder space/new.txt"), b"new").unwrap();
    upload.items.remove(0);
    run(&upload);
    assert_eq!(
        fs::read(destination.join("folder space/new.txt")).unwrap(),
        b"new"
    );
    assert!(!destination.join("folder space/folder space").exists());
    let transfer = Transfer {
        connection,
        upload: false,
        items: vec![
            TransferItem {
                local: download.join(name),
                remote: RemotePath::from_path(&destination.join(name)),
                directory: false,
            },
            TransferItem {
                local: download.join("folder space"),
                remote: RemotePath::from_path(&destination.join("folder space")),
                directory: true,
            },
        ],
    };
    run(&transfer);
    assert_eq!(fs::read(download.join(name)).unwrap(), b"literal payload");
    assert_eq!(
        fs::read(download.join("folder space/nested.txt")).unwrap(),
        b"nested"
    );
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn directory_probe_preserves_names_and_reports_missing_folders() {
    use std::{fs, os::unix::ffi::OsStrExt, process::Command};
    let root = std::env::temp_dir().join(format!("ely-remote-list-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    #[cfg(target_os = "macos")]
    let name = b"raw\nname".as_slice();
    #[cfg(not(target_os = "macos"))]
    let name = b"raw\xff\nname".as_slice();
    fs::write(root.join(OsStr::from_bytes(name)), b"data").unwrap();
    let request =
        serde_json::to_vec(&serde_json::json!({"path": RemotePath::from_path(&root)})).unwrap();
    let mut command = Command::new("python3");
    command.args(["-c", SCRIPT]);
    let out = crate::ssh::remote::process::output(
        &mut command,
        &request,
        &AtomicBool::new(false),
        Duration::from_secs(5),
    )
    .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let listing: Listing = serde_json::from_slice(&out.stdout).unwrap();
    assert!(listing.entries.iter().any(|entry| entry.name.0 == name));
    fs::remove_dir_all(root).unwrap();
    let out = crate::ssh::remote::process::output(
        &mut command,
        &request,
        &AtomicBool::new(false),
        Duration::from_secs(5),
    )
    .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("remote files:"));
}
