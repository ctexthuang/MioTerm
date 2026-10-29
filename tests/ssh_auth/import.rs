use std::{fs, path::Path, process::Command};

use elyterminal::ssh::{Authentication, Host, auth};

pub fn run() {
    let root = std::env::var_os("MIOTERM_CONFIG_DIR").unwrap();
    let root = Path::new(&root);
    let source = root.join("selected private key.pem");
    let passphrase = "fixture PEM passphrase";
    assert!(
        Command::new("ssh-keygen")
            .args([
                "-q", "-t", "rsa", "-b", "2048", "-m", "PEM", "-N", passphrase, "-f"
            ])
            .arg(&source)
            .output()
            .unwrap()
            .status
            .success()
    );
    let bytes = fs::read(&source).unwrap();
    assert!(bytes.starts_with(b"-----BEGIN RSA PRIVATE KEY-----"));
    let public = fs::read(source.with_extension("pem.pub")).unwrap();
    let original = Host {
        address: "pem-import.fixture.invalid".into(),
        identity: source.display().to_string(),
        auth: Authentication::PrivateKey,
        remember_secret: true,
        ..Host::default()
    };
    auth::save(&original, passphrase).unwrap();
    let imported = original.clone().prepare(&original, "").unwrap();
    assert_ne!(auth::account(&original), auth::account(&imported));
    assert!(Path::new(&imported.identity).starts_with(root.join("keys")));
    assert_eq!(fs::read(&source).unwrap(), bytes);
    assert_eq!(fs::read(&imported.identity).unwrap(), bytes);
    assert_eq!(auth::load(&imported).unwrap().unwrap().as_str(), passphrase);
    assert_eq!(auth::load(&original).unwrap().unwrap().as_str(), passphrase);
    assert!(
        !serde_json::to_string(&imported)
            .unwrap()
            .contains(passphrase)
    );
    let repeated = original.clone().prepare(&original, "").unwrap();
    assert_eq!(repeated.identity, imported.identity);
    let renamed = root.join("renamed.pem");
    fs::copy(&source, &renamed).unwrap();
    let repeated = Host {
        identity: renamed.display().to_string(),
        ..original.clone()
    }
    .prepare(&original, "")
    .unwrap();
    assert_eq!(repeated.identity, imported.identity);
    assert_eq!(auth::load(&repeated).unwrap().unwrap().as_str(), passphrase);

    let different = root.join("different.pem");
    assert!(
        Command::new("ssh-keygen")
            .args(["-q", "-t", "ed25519", "-N", "", "-f"])
            .arg(&different)
            .output()
            .unwrap()
            .status
            .success()
    );
    let changed = Host {
        identity: different.display().to_string(),
        ..original.clone()
    }
    .prepare(&original, "")
    .unwrap();
    assert!(auth::load(&changed).unwrap().is_none());

    auth::save(&imported, "different saved fixture").unwrap();
    assert!(original.clone().prepare(&original, "").is_err());
    assert_eq!(
        auth::load(&imported).unwrap().unwrap().as_str(),
        "different saved fixture"
    );
    auth::save(&imported, passphrase).unwrap();

    let invalid = root.join("public-only.pub");
    fs::write(&invalid, &public).unwrap();
    assert!(
        Host {
            identity: invalid.display().to_string(),
            ..original.clone()
        }
        .prepare(&original, "")
        .is_err()
    );
    assert_eq!(auth::load(&original).unwrap().unwrap().as_str(), passphrase);

    fs::remove_file(source).unwrap();
    let edited = imported.clone().prepare(&imported, "").unwrap();
    assert_eq!(edited.identity, imported.identity);
    let output = Command::new("ssh-keygen")
        .args(["-y", "-P", passphrase, "-f", &edited.identity])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(fields(&output.stdout), fields(&public));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&imported.identity)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(root.join("keys"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }
    auth::forget(&original).unwrap();
    auth::forget(&imported).unwrap();
    println!("ssh_auth: RSA PEM import, deleted source, passphrase migration and conflicts passed");
}

fn fields(bytes: &[u8]) -> Vec<&[u8]> {
    bytes
        .split(|byte| byte.is_ascii_whitespace())
        .take(2)
        .collect()
}
