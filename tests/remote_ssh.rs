#![cfg(unix)]

use std::{fs, path::PathBuf, process::Command, sync::atomic::AtomicBool};

use ely_gpui_component::git::ChangeSection;
use elyterminal::{
    git::Backend,
    ssh::{
        Host,
        files::{RemotePath, Transfer, TransferItem},
        remote::Remote,
    },
};

#[test]
#[ignore = "requires an isolated SSH master in ELY_SSH_TEST_SOCKET"]
fn remote_git_and_sftp_use_the_authenticated_connection() {
    let remote = Remote {
        session: 0,
        host: Host {
            address: "127.0.0.1".into(),
            port: std::env::var("ELY_SSH_TEST_PORT").unwrap().parse().unwrap(),
            user: std::env::var("USER").unwrap(),
            ..Host::default()
        },
        socket: Some(std::env::var_os("ELY_SSH_TEST_SOCKET").unwrap().into()),
    };
    let root =
        PathBuf::from("/private/tmp").join(format!("ely-remote-check-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let repo = root.join("repo ' with spaces");
    fs::create_dir(&repo).unwrap();
    let git = |args: &[&str]| {
        let out = Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    };
    git(&["init", "-q", "-b", "main"]);
    for (key, value) in [
        ("user.name", "SSH Fixture"),
        ("user.email", "fixture@example.invalid"),
        ("commit.gpgsign", "false"),
        ("core.hooksPath", "/dev/null"),
    ] {
        git(&["config", key, value]);
    }
    let filename = "a '*[x]$.txt";
    fs::write(repo.join(filename), b"first\n").unwrap();
    let backend = Backend::Remote(remote.clone());
    let state = backend.repo(&repo).unwrap().unwrap();
    assert_eq!(state.top, repo);
    assert_eq!(state.untracked.len(), 1);
    assert!(
        backend
            .diff(&repo, &state.untracked[0], ChangeSection::Untracked)
            .unwrap()
            .is_some()
    );
    backend.stage(&repo, &state.untracked).unwrap();
    let staged = backend.repo(&repo).unwrap().unwrap().staged;
    assert_eq!(staged.len(), 1);
    backend.unstage(&repo, &staged).unwrap();
    assert_eq!(backend.repo(&repo).unwrap().unwrap().untracked.len(), 1);
    backend.stage(&repo, &staged).unwrap();
    backend.commit(&repo, "first remote commit", false).unwrap();
    assert_eq!(
        backend.log(&repo, 50).unwrap()[0].subject.as_ref(),
        "first remote commit"
    );
    fs::write(repo.join(filename), b"edited\n").unwrap();
    let unstaged = backend.repo(&repo).unwrap().unwrap().unstaged;
    assert_eq!(unstaged.len(), 1);
    assert!(
        backend
            .diff(&repo, &unstaged[0], ChangeSection::Unstaged)
            .unwrap()
            .is_some()
    );
    backend
        .discard(&repo, &unstaged[0], ChangeSection::Unstaged)
        .unwrap();
    assert_eq!(fs::read(repo.join(filename)).unwrap(), b"first\n");
    let cancel = AtomicBool::new(false);
    let listing = remote
        .list(Some(&RemotePath::from_path(&repo)), &cancel)
        .unwrap();
    assert!(
        listing
            .entries
            .iter()
            .any(|entry| entry.name.0 == filename.as_bytes())
    );
    let source = root.join("upload '[x]*.txt");
    fs::write(&source, b"sftp over SSH").unwrap();
    let target = root.join("uploaded '[x]*.txt");
    Transfer {
        connection: remote.clone(),
        upload: true,
        items: vec![TransferItem {
            local: source,
            remote: RemotePath::from_path(&target),
            directory: false,
        }],
    }
    .run(&cancel)
    .unwrap();
    assert_eq!(fs::read(&target).unwrap(), b"sftp over SSH");
    let download = root.join("downloaded.txt");
    Transfer {
        connection: remote,
        upload: false,
        items: vec![TransferItem {
            local: download.clone(),
            remote: RemotePath::from_path(&target),
            directory: false,
        }],
    }
    .run(&cancel)
    .unwrap();
    assert_eq!(fs::read(download).unwrap(), b"sftp over SSH");
    fs::remove_dir_all(root).unwrap();
}

#[test]
#[ignore = "requires an isolated loopback SSH master in ELY_SSH_TEST_SOCKET"]
fn remote_shell_reports_cd_over_an_actual_ssh_channel() {
    use std::{ffi::OsStr, time::Duration};
    let remote = Remote {
        host: Host {
            port: std::env::var("ELY_SSH_TEST_PORT").unwrap().parse().unwrap(),
            ..Host::default()
        },
        socket: Some(std::env::var_os("ELY_SSH_TEST_SOCKET").unwrap().into()),
        session: 0,
    };
    let home = std::env::temp_dir().join(format!("ely-ssh-shell-home-{}", std::process::id()));
    fs::create_dir_all(home.join("second")).unwrap();
    fs::write(home.join(".bash_profile"), "PS1='fixture> '\n").unwrap();
    let home_env = format!("HOME={}", home.display());
    let command = elyterminal::ssh::shell::command();
    let args = [
        "env",
        &home_env,
        "SHELL=/bin/bash",
        "/bin/sh",
        "-c",
        &command,
    ];
    let input = format!("cd '{}'\ncd second\nexit\n", home.display());
    let output = remote
        .output(
            &args.map(OsStr::new),
            input.as_bytes(),
            &AtomicBool::new(false),
            Duration::from_secs(15),
        )
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains(&format!("\x1b]7;file://{}/second\x07", home.display())),
        "{text:?}"
    );
    fs::remove_dir_all(home).unwrap();
}
