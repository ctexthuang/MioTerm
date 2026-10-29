use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use super::*;

pub(in crate::ssh) struct Folder(pub(in crate::ssh) PathBuf);

impl Folder {
    pub(in crate::ssh) fn new() -> Self {
        static COUNT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "mioterm-vault-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            COUNT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn encrypted_file_survives_restart_without_plaintext_secrets_or_account_names() {
    let folder = Folder::new();
    let path = folder.0.join("credentials");
    let vault = Vault::at(path.clone());
    vault.put("sync:gitee", "fixture-token-only").unwrap();
    vault.put("sync:archive", "635841").unwrap();
    let bytes = fs::read(path.join("credentials.age")).unwrap();
    assert!(bytes.starts_with(b"age-encryption.org/v1\n"));
    for secret in [b"fixture-token-only".as_slice(), b"635841", b"sync:gitee"] {
        assert!(!bytes.windows(secret.len()).any(|window| window == secret));
    }
    let restarted = Vault::at(path.clone());
    assert_eq!(
        restarted.get("sync:archive").unwrap().unwrap().as_str(),
        "635841"
    );
    restarted.remove("sync:archive").unwrap();
    assert!(vault.get("sync:archive").unwrap().is_none());
    assert_eq!(
        vault.get("sync:gitee").unwrap().unwrap().as_str(),
        "fixture-token-only"
    );
    assert_eq!(fs::read_dir(&path).unwrap().count(), 3);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o700
        );
        for name in ["credentials.age", "local.key", ".lock"] {
            assert_eq!(
                fs::metadata(path.join(name)).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }
}

#[test]
fn different_devices_use_different_local_keys_and_a_copied_vault_needs_its_key() {
    let folder = Folder::new();
    let source = Vault::at(folder.0.join("source"));
    let destination = Vault::at(folder.0.join("destination"));
    source.put("ssh:fixture", "source-password").unwrap();
    destination
        .put("ssh:fixture", "destination-password")
        .unwrap();
    let before = fs::read(destination.folder.join("credentials.age")).unwrap();
    assert_ne!(
        fs::read(source.folder.join("local.key")).unwrap(),
        fs::read(destination.folder.join("local.key")).unwrap()
    );
    fs::copy(
        source.folder.join("credentials.age"),
        destination.folder.join("credentials.age"),
    )
    .unwrap();
    assert!(destination.get("ssh:fixture").is_err());
    let wrong_key_file = fs::read(destination.folder.join("credentials.age")).unwrap();
    assert!(destination.put("ssh:other", "must-not-overwrite").is_err());
    assert_eq!(
        wrong_key_file,
        fs::read(destination.folder.join("credentials.age")).unwrap()
    );
    fs::copy(
        source.folder.join("local.key"),
        destination.folder.join("local.key"),
    )
    .unwrap();
    assert_eq!(
        destination.get("ssh:fixture").unwrap().unwrap().as_str(),
        "source-password"
    );
    assert_ne!(before, wrong_key_file);
}

#[test]
fn missing_keys_and_tampered_files_fail_without_resetting_the_vault() {
    let folder = Folder::new();
    let vault = Vault::at(folder.0.join("vault"));
    vault.put("ssh:fixture", "saved-fixture-password").unwrap();
    let path = vault.folder.join("credentials.age");
    let mut original = fs::read(&path).unwrap();
    let key = fs::read(vault.folder.join("local.key")).unwrap();
    fs::remove_file(vault.folder.join("local.key")).unwrap();
    assert!(vault.put("ssh:other", "must-not-reset").is_err());
    assert!(!vault.folder.join("local.key").exists());
    assert_eq!(original, fs::read(&path).unwrap());
    fs::write(vault.folder.join("local.key"), key).unwrap();
    let last = original.len() - 1;
    original[last] ^= 1;
    fs::write(&path, &original).unwrap();
    assert!(vault.get("ssh:fixture").is_err());
    assert!(vault.remove("ssh:fixture").is_err());
    assert_eq!(original, fs::read(path).unwrap());
}

#[test]
fn overlapping_writers_and_imports_preserve_other_credentials() {
    let folder = Folder::new();
    let path = folder.0.join("vault");
    let writers = (0..8)
        .map(|writer| {
            let path = path.clone();
            std::thread::spawn(move || {
                let vault = Vault::at(path);
                for index in 0..8 {
                    vault
                        .put(&format!("fixture:{writer}:{index}"), "fixture-value")
                        .unwrap();
                }
                vault.insert("shared-fixture", "same-value").unwrap();
            })
        })
        .collect::<Vec<_>>();
    for writer in writers {
        writer.join().unwrap();
    }
    let vault = Vault::at(path.clone());
    assert_eq!(vault.read(|contents| contents.entries.len()).unwrap(), 65);
    assert!(vault.insert("shared-fixture", "different-value").is_err());
    assert_eq!(
        vault.get("shared-fixture").unwrap().unwrap().as_str(),
        "same-value"
    );
    assert_eq!(fs::read_dir(path).unwrap().count(), 3);
}

#[cfg(unix)]
#[test]
fn symlinks_and_hardlinks_cannot_redirect_private_reads_or_writes() {
    use std::os::unix::fs::symlink;
    let folder = Folder::new();
    let victim = folder.0.join("victim");
    fs::write(&victim, "untouched").unwrap();
    for name in ["local.key", "credentials.age", ".lock"] {
        let path = folder.0.join(name);
        fs::create_dir(&path).unwrap();
        symlink(&victim, path.join(name)).unwrap();
        let vault = Vault::at(path);
        assert!(vault.put("fixture", "must-not-write").is_err());
        assert_eq!(fs::read_to_string(&victim).unwrap(), "untouched");
    }
    let path = folder.0.join("hardlink");
    fs::create_dir(&path).unwrap();
    fs::hard_link(&victim, path.join("local.key")).unwrap();
    assert!(Vault::at(path).get("fixture").is_err());
    let path = folder.0.join("symlink-folder");
    symlink(&folder.0, &path).unwrap();
    assert!(Vault::at(path).put("fixture", "must-not-write").is_err());
}
