use std::{ffi::OsStr, process::Command};

use super::Keepalive;
use crate::ssh::{Connections, Host, auth, control::Control, files::Transfer, remote::Remote};

struct Folder(std::path::PathBuf);

impl Folder {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("mioterm-keepalive-{}-{stamp}", std::process::id()));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn saved_hosts_load_defaults_and_preserve_disabled_intervals() {
    let json = serde_json::to_value(Host::default()).unwrap();
    assert!(json.get("keepalive").is_none());
    let loaded: Host = serde_json::from_value(json).unwrap();
    assert_eq!(loaded.keepalive, Keepalive::default());
    for enabled in [true, false] {
        let host = Host {
            keepalive: Keepalive {
                enabled,
                interval: 75,
            },
            ..Host::default()
        };
        let encoded = serde_json::to_string(&host).unwrap();
        let loaded: Host = serde_json::from_str(&encoded).unwrap();
        assert_eq!(loaded, host);
    }
}

#[test]
fn invalid_intervals_are_refused_even_while_disabled() {
    for enabled in [true, false] {
        for interval in [0, 3601, u16::MAX] {
            let host = Host {
                keepalive: Keepalive { enabled, interval },
                ..Host::default()
            };
            assert!(host.validate().is_err());
        }
        for interval in [Keepalive::MIN, Keepalive::MAX] {
            assert!(Keepalive { enabled, interval }.validate().is_ok());
        }
    }
}

fn check_options(command: &Command, enabled: bool, interval: u16) {
    let args: Vec<_> = command
        .get_args()
        .map(|arg| arg.to_string_lossy())
        .collect();
    let option = format!("ServerAliveInterval={}", if enabled { interval } else { 0 });
    assert_eq!(args.iter().filter(|arg| **arg == option).count(), 1);
    assert_eq!(
        args.iter()
            .filter(|arg| **arg == "ServerAliveCountMax=3")
            .count(),
        1
    );
    let destination = args.iter().position(|arg| *arg == "--").unwrap();
    assert!(args.iter().position(|arg| **arg == option).unwrap() < destination);
}

#[test]
fn terminal_control_master_background_commands_and_sftp_share_keepalive() {
    for enabled in [true, false] {
        let host = Host {
            keepalive: Keepalive {
                enabled,
                interval: 75,
            },
            ..Host::default()
        };
        let mut launch = host.launch(std::env::temp_dir()).unwrap();
        let control = Control::new().unwrap();
        control.configure(&mut launch);
        let (program, args) = launch.program.unwrap();
        let mut command = Command::new(program);
        command.args(args);
        check_options(&command, enabled, 75);
        #[cfg(unix)]
        assert!(command.get_args().any(|arg| arg == "ControlMaster=yes"));
        let remote = Remote {
            host,
            session: 1,
            socket: control.socket(),
        };
        check_options(&remote.command(&[OsStr::new("pwd")]).unwrap(), enabled, 75);
        let transfer = Transfer {
            connection: remote,
            upload: true,
            items: Vec::new(),
        };
        check_options(&transfer.command().unwrap(), enabled, 75);
    }
}

#[test]
fn openssh_resolves_explicit_on_and_off_without_reading_user_configuration() {
    let folder = Folder::new();
    let config = folder.0.join("ssh-config");
    std::fs::write(
        &config,
        "Host *\n ServerAliveInterval 999\n ServerAliveCountMax 9\n",
    )
    .unwrap();
    let program = crate::settings::program_path("ssh").unwrap();
    for enabled in [true, false] {
        let host = Host {
            keepalive: Keepalive {
                enabled,
                interval: 75,
            },
            ..Host::default()
        };
        let out = Command::new(&program)
            .arg("-G")
            .arg("-F")
            .arg(&config)
            .args(host.arguments().unwrap())
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let resolved = String::from_utf8(out.stdout).unwrap();
        let interval = if enabled { 75 } else { 0 };
        assert!(
            resolved
                .lines()
                .any(|line| line == format!("serveraliveinterval {interval}"))
        );
        assert!(resolved.lines().any(|line| line == "serveralivecountmax 3"));
    }
}

#[test]
fn editing_keepalive_updates_recents_without_changing_credentials_or_deletions() {
    let mut local = Connections::default();
    let host = local.upsert(Host::default()).unwrap();
    local.opened(host.clone(), 1);
    let changed = Host {
        keepalive: Keepalive {
            enabled: false,
            interval: 120,
        },
        ..host.clone()
    };
    assert_eq!(auth::account(&host), auth::account(&changed));
    local.upsert(changed.clone()).unwrap();
    assert_eq!(local.recent[0].host, changed);
    local.opened(host.clone(), 2);
    assert_eq!(local.recent.len(), 1);
    local.remove(host.id).unwrap();
    assert!(local.is_deleted(&changed, None));
}
