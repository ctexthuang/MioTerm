use std::{
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

use elyterminal::ssh::{Authentication, Host, auth};

struct Fixture(Host);

impl Drop for Fixture {
    fn drop(&mut self) {
        auth::forget(&self.0).expect("ssh auth test: remove fixture credential");
    }
}

fn main() {
    if let Some(code) = auth::askpass::run() {
        std::process::exit(code);
    }
    if !std::env::args().any(|arg| arg == "--ignored") {
        println!("ssh_auth: skipped; --ignored opts into one temporary OS credential");
        return;
    }
    saved_password_uses_the_real_helper_without_exposing_it_in_the_profile();
    #[cfg(unix)]
    if std::env::var_os("ELY_SSH_TEST_IDENTITY").is_some() {
        encrypted_private_key_logs_in_without_an_agent_or_control_socket();
    }
    println!("ssh_auth: system credential and askpass round-trip passed");
}

#[cfg(unix)]
fn encrypted_private_key_logs_in_without_an_agent_or_control_socket() {
    use elyterminal::ssh::{
        files::{RemotePath, Transfer, TransferItem},
        remote::{Remote, process},
    };
    use std::{ffi::OsStr, fs, sync::atomic::AtomicBool, time::Duration};
    let root = std::env::temp_dir().join(format!("ely-ssh-auth-key-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let identity = root.join("encrypted_key");
    fs::copy(
        std::env::var_os("ELY_SSH_TEST_IDENTITY").unwrap(),
        &identity,
    )
    .unwrap();
    let passphrase = "temporary fixture key passphrase";
    assert!(
        Command::new("ssh-keygen")
            .args(["-q", "-p", "-P", "", "-N", passphrase, "-f"])
            .arg(&identity)
            .output()
            .unwrap()
            .status
            .success()
    );
    let fixture = Fixture(Host {
        port: std::env::var("ELY_SSH_TEST_PORT").unwrap().parse().unwrap(),
        user: std::env::var("USER").unwrap(),
        auth: Authentication::PrivateKey,
        identity: identity.display().to_string(),
        remember_secret: true,
        ..Host::default()
    });
    auth::save(&fixture.0, passphrase).unwrap();
    let known = std::env::var_os("ELY_SSH_TEST_KNOWN_HOSTS").unwrap();
    let isolate = |source: Command| {
        let mut command = Command::new(source.get_program());
        command
            .args(["-F", "none", "-o", "IdentityAgent=none", "-o"])
            .arg({
                let mut option = std::ffi::OsString::from("UserKnownHostsFile=");
                option.push(&known);
                option
            })
            .args(source.get_args());
        for (key, value) in source.get_envs() {
            if let Some(value) = value {
                command.env(key, value);
            } else {
                command.env_remove(key);
            }
        }
        command
    };
    let remote = Remote {
        host: fixture.0.clone(),
        socket: None,
        session: 0,
    };
    let cancel = AtomicBool::new(false);
    let output = process::output(
        &mut isolate(
            remote
                .command(&[OsStr::new("printf"), OsStr::new("key-authenticated")])
                .unwrap(),
        ),
        b"",
        &cancel,
        Duration::from_secs(20),
    )
    .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"key-authenticated");
    let local = root.join("source.txt");
    fs::write(&local, b"saved key passphrase via SFTP").unwrap();
    let target = root.join("target.txt");
    let transfer = Transfer {
        connection: remote,
        upload: true,
        items: vec![TransferItem {
            local,
            remote: RemotePath::from_path(&target),
            directory: false,
        }],
    };
    let output = process::output(
        &mut isolate(transfer.command().unwrap()),
        &transfer.batch().unwrap(),
        &cancel,
        Duration::from_secs(20),
    )
    .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read(&target).unwrap(), b"saved key passphrase via SFTP");
    drop(fixture);
    fs::remove_dir_all(root).unwrap();
}

fn saved_password_uses_the_real_helper_without_exposing_it_in_the_profile() {
    let fixture = Fixture(Host {
        address: format!(
            "fixture-{}-{}.invalid",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ),
        auth: Authentication::Password,
        remember_secret: true,
        ..Host::default()
    });
    let secret = "fixture password with spaces";
    auth::save(&fixture.0, secret).unwrap();
    assert_eq!(
        auth::load(&fixture.0)
            .unwrap()
            .as_deref()
            .map(String::as_str),
        Some(secret)
    );
    assert!(!serde_json::to_string(&fixture.0).unwrap().contains(secret));
    let mut helper = Command::new(std::env::current_exe().unwrap());
    auth::configure_command(&fixture.0, &mut helper).unwrap();
    helper
        .arg("fixture@localhost's password: ")
        .stdin(Stdio::null());
    let output = elyterminal::ssh::remote::process::output(
        &mut helper,
        b"",
        &std::sync::atomic::AtomicBool::new(false),
        std::time::Duration::from_secs(20),
    )
    .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, format!("{secret}\n").as_bytes());
    let mut helper = Command::new(std::env::current_exe().unwrap());
    auth::configure_command(&fixture.0, &mut helper).unwrap();
    helper.arg("Verification code: ").stdin(Stdio::null());
    let output = elyterminal::ssh::remote::process::output(
        &mut helper,
        b"",
        &std::sync::atomic::AtomicBool::new(false),
        std::time::Duration::from_secs(20),
    )
    .unwrap();
    assert!(!output.status.success());
    assert!(!String::from_utf8_lossy(&output.stderr).contains(secret));
    assert!(output.stdout.is_empty());
    auth::save(&fixture.0, "").unwrap();
    auth::forget(&fixture.0).unwrap();
    assert!(auth::load(&fixture.0).unwrap().is_none());
}
