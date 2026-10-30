use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use age::secrecy::SecretString;

use super::*;

pub(super) struct Folder(pub(super) PathBuf);

impl Folder {
    pub(super) fn new() -> Self {
        static COUNT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "ely-webdav-{}-{}",
            std::process::id(),
            COUNT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    pub(super) fn local(&self, name: &str) -> Connections {
        let key = self.0.join(name);
        fs::write(&key, format!("-----BEGIN OPENSSH PRIVATE KEY-----\nTEST FIXTURE {name}\n-----END OPENSSH PRIVATE KEY-----\n")).unwrap();
        let mut data = Connections::default();
        data.upsert(Host {
            name: name.into(),
            address: "example.com".into(),
            user: "debian".into(),
            identity: key.to_str().unwrap().into(),
            ..Host::default()
        })
        .unwrap();
        data
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn settings_accept_only_https_folder_urls_without_embedded_secrets() {
    for url in [
        "http://example.com/dav",
        "file:///tmp/archive",
        "https://user:secret@example.com/dav",
        "https://example.com/dav?token=secret",
        "https://example.com/dav#fragment",
        "not a url",
    ] {
        assert!(
            WebDav {
                url: url.into(),
                ..WebDav::default()
            }
            .validate()
            .is_err(),
            "{url}"
        );
    }
    for url in ["https://example.com/dav/", "https://example.com/dav"] {
        let settings = WebDav {
            url: url.into(),
            ..WebDav::default()
        };
        assert_eq!(
            settings.endpoint().unwrap().as_str(),
            "https://example.com/dav/elyterminal-ssh.age"
        );
    }
    assert!(
        WebDav {
            username: "a:b".into(),
            ..WebDav::default()
        }
        .validate()
        .is_err()
    );
    assert!(WebDav::default().validate().is_ok());
}

#[test]
fn settings_never_serialize_passwords_and_old_settings_still_load() {
    let settings: crate::settings::Settings = serde_json::from_str("{}").unwrap();
    assert_eq!(settings.webdav, WebDav::default());
    let json = serde_json::to_string(&settings.webdav).unwrap();
    assert!(!json.contains("password") && !json.contains("passphrase"));
    assert!(serde_json::from_str::<WebDav>(r#"{"password":"not allowed"}"#).is_err());
}

#[test]
fn archives_contain_key_material_instead_of_machine_specific_paths_or_recent_history() {
    let folder = Folder::new();
    let mut local = folder.local("source-key");
    local.opened(
        Host {
            address: "private-history.example".into(),
            ..Host::default()
        },
        1,
    );
    let archive = Archive::capture(&local).unwrap();
    assert_eq!(archive.keys.len(), 1);
    let json = serde_json::to_string(&archive).unwrap();
    assert!(!json.contains(&folder.0.display().to_string()));
    assert!(!json.contains("private-history"));
    let restored = archive
        .restore(Connections::default(), &folder.0.join("new-machine"))
        .unwrap();
    let path = &restored.hosts[0].identity;
    assert_ne!(path, &local.hosts[0].identity);
    assert_eq!(
        fs::read(path).unwrap(),
        fs::read(&local.hosts[0].identity).unwrap()
    );
    assert_eq!(restored.hosts[0].id, 1);
}

#[test]
fn portable_hosts_validate_without_preserving_a_machine_specific_key_path() {
    let folder = Folder::new();
    let mut local = folder.local("portable-key");
    local.hosts[0].auth = crate::ssh::Authentication::PrivateKey;
    let archive = Archive::capture(&local).unwrap();
    let value = serde_json::to_value(&archive).unwrap();
    assert!(value["hosts"][0]["key"].as_str().is_some());
    assert!(value["hosts"][0].get("identity").is_none());
    assert!(archive.validate().is_ok());
    let mut missing_key = value.clone();
    missing_key["hosts"][0]["key"] = serde_json::Value::Null;
    missing_key["keys"] = serde_json::json!({});
    let invalid: Archive = serde_json::from_value(missing_key).unwrap();
    assert!(invalid.validate().is_err());
    let mut bad_host = value;
    bad_host["hosts"][0]["address"] = serde_json::json!("-invalid");
    let invalid: Archive = serde_json::from_value(bad_host).unwrap();
    assert!(invalid.validate().is_err());
}

#[test]
fn encryption_passphrases_require_at_least_six_characters() {
    for value in ["", "a", "abcde", "12345", "测试口令短"] {
        assert!(crypto::validate_passphrase(&SecretString::from(value.to_string())).is_err());
    }
    for value in [
        "abcdef",
        "123456",
        "abcdefg",
        "abcdefghijkl",
        "测试口令六字",
    ] {
        assert!(crypto::validate_passphrase(&SecretString::from(value.to_string())).is_ok());
    }
}

#[test]
fn empty_encryption_passphrases_ask_for_input_instead_of_reporting_a_length_error() {
    let error = crypto::validate_passphrase(&SecretString::from(String::new())).unwrap_err();
    assert_eq!(
        error.to_string(),
        "SSH sync: enter the encryption passphrase or enable remembrance and save it first"
    );
    let error = crypto::validate_passphrase(&SecretString::from("12345".to_string())).unwrap_err();
    assert_eq!(
        error.to_string(),
        "SSH sync: the encryption passphrase must have at least 6 characters"
    );
}

#[test]
fn the_encrypted_bundle_round_trips_and_rejects_wrong_passwords_and_tampering() {
    let folder = Folder::new();
    let archive = Archive::capture(&folder.local("secret-key")).unwrap();
    let password = SecretString::from("abcdef".to_string());
    let mut encrypted = crypto::encrypt(&archive, &password).unwrap();
    assert!(encrypted.starts_with(b"age-encryption.org/v1\n"));
    assert!(!encrypted.windows(11).any(|part| part == b"example.com"));
    assert!(!encrypted.windows(12).any(|part| part == b"TEST FIXTURE"));
    let decoded = crypto::decrypt(&encrypted, &password).unwrap();
    assert_eq!(decoded.hosts.len(), 1);
    assert_eq!(decoded.keys.len(), 1);
    assert!(
        crypto::decrypt(
            &encrypted,
            &SecretString::from("a different test password".to_string())
        )
        .is_err()
    );
    *encrypted.last_mut().unwrap() ^= 1;
    assert!(crypto::decrypt(&encrypted, &password).is_err());
    assert!(crypto::validate_passphrase(&SecretString::from("short".to_string())).is_err());
    assert!(crypto::decrypt(b"not encrypted", &password).is_err());
}

#[test]
fn merge_keeps_variants_and_deduplicates_identical_hosts_and_shared_keys() {
    let folder = Folder::new();
    let local = folder.local("shared-key");
    let mut merged = Archive::capture(&local).unwrap();
    merged.merge(Archive::capture(&local).unwrap()).unwrap();
    assert_eq!(merged.hosts.len(), 1);
    let mut other = local.clone();
    other.hosts[0].user = "root".into();
    merged.merge(Archive::capture(&other).unwrap()).unwrap();
    assert_eq!(merged.hosts.len(), 2);
    assert_eq!(merged.keys.len(), 1);
    let restored = merged.restore(local, &folder.0.join("managed")).unwrap();
    assert_eq!(restored.hosts.len(), 2);
    let twice = merged
        .restore(restored.clone(), &folder.0.join("managed"))
        .unwrap();
    assert_eq!(twice, restored);
}

#[test]
fn missing_keys_fail_capture_and_a_restore_repairs_a_stale_key_path() {
    let folder = Folder::new();
    let local = folder.local("old-machine-key");
    let archive = Archive::capture(&local).unwrap();
    fs::remove_file(&local.hosts[0].identity).unwrap();
    assert!(Archive::capture(&local).is_err());
    let restored = archive
        .restore(local.clone(), &folder.0.join("managed"))
        .unwrap();
    assert_eq!(restored.hosts.len(), 1);
    assert_eq!(restored.hosts[0].id, local.hosts[0].id);
    assert!(PathBuf::from(&restored.hosts[0].identity).is_file());
}

#[test]
fn applying_sync_never_overwrites_a_host_edited_while_the_network_was_busy() {
    let folder = Folder::new();
    let local = folder.local("local");
    let mut edited = local.clone();
    edited.hosts[0].name = "Edited during sync".into();
    let restored = Restored {
        before: local.hosts.clone(),
        before_next_id: local.next_id,
        before_deleted: local.deleted_hosts.clone(),
        connections: local,
        hosts: 1,
        keys: 1,
        credentials: 0,
        warnings: Vec::new(),
    };
    assert!(restored.apply(&mut edited).is_err());
    assert_eq!(edited.hosts[0].name, "Edited during sync");
}

#[test]
fn applying_sync_preserves_allocated_ids_and_does_not_reassign_an_old_recent_target() {
    let folder = Folder::new();
    let mut local = folder.local("local");
    local.opened(
        Host {
            address: "old-target.example".into(),
            ..local.hosts[0].clone()
        },
        1,
    );
    let mut imported = local.clone();
    imported.hosts[0].identity = folder.0.join("restored").to_str().unwrap().into();
    let restored = || Restored {
        before: local.hosts.clone(),
        before_next_id: local.next_id,
        before_deleted: local.deleted_hosts.clone(),
        connections: imported.clone(),
        hosts: 1,
        keys: 1,
        credentials: 0,
        warnings: Vec::new(),
    };
    let mut allocated = local.clone();
    allocated.next_id += 1;
    assert!(restored().apply(&mut allocated).is_err());
    let mut current = local.clone();
    restored().apply(&mut current).unwrap();
    assert_eq!(
        current.recent[0].host.identity,
        local.recent[0].host.identity
    );
    assert_eq!(current.hosts[0].identity, imported.hosts[0].identity);
}

#[test]
fn identityless_hosts_remain_identityless_and_import_ids_do_not_collide() {
    let folder = Folder::new();
    let mut local = folder.local("local-key");
    let mut other = Connections::default();
    other
        .upsert(Host {
            address: "other.example".into(),
            ..Host::default()
        })
        .unwrap();
    let archive = Archive::capture(&other).unwrap();
    local = archive.restore(local, &folder.0.join("managed")).unwrap();
    assert_eq!(local.hosts.len(), 2);
    assert_ne!(local.hosts[0].id, local.hosts[1].id);
    assert!(local.hosts[1].identity.is_empty());
}

#[test]
fn missing_referenced_keys_unknown_fields_and_mismatched_key_digests_are_rejected() {
    let folder = Folder::new();
    let archive = Archive::capture(&folder.local("key")).unwrap();
    let mut value = serde_json::to_value(&archive).unwrap();
    value["keys"] = serde_json::json!({});
    let broken: Archive = serde_json::from_value(value).unwrap();
    assert!(broken.validate().is_err());
    let mut value = serde_json::to_value(&archive).unwrap();
    value["hosts"][0]["key"] = serde_json::json!("../../escape");
    let broken: Archive = serde_json::from_value(value).unwrap();
    assert!(broken.validate().is_err());
    let mut value = serde_json::to_value(&archive).unwrap();
    value["shell_command"] = serde_json::json!("not allowed");
    assert!(serde_json::from_value::<Archive>(value).is_err());
    let mut value = serde_json::to_value(&archive).unwrap();
    let id = archive.keys.keys().next().unwrap();
    value["keys"][id]["private"] = serde_json::json!("malformed");
    let broken: Archive = serde_json::from_value(value).unwrap();
    assert!(broken.validate().is_err());
}

#[test]
fn restored_keys_are_restricted_and_existing_files_are_never_overwritten() {
    let folder = Folder::new();
    let archive = Archive::capture(&folder.local("source")).unwrap();
    let managed = folder.0.join("managed");
    let restored = archive.restore(Connections::default(), &managed).unwrap();
    let path = PathBuf::from(&restored.hosts[0].identity);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(&managed).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }
    fs::write(&path, "not the restored key").unwrap();
    assert!(archive.restore(Connections::default(), &managed).is_err());
    assert_eq!(fs::read_to_string(path).unwrap(), "not the restored key");
}

#[cfg(unix)]
#[test]
fn symlinks_cannot_redirect_managed_key_writes() {
    use std::os::unix::fs::symlink;
    let folder = Folder::new();
    let archive = Archive::capture(&folder.local("source")).unwrap();
    let victim = folder.0.join("victim");
    fs::write(&victim, "untouched").unwrap();
    let managed = folder.0.join("managed");
    fs::create_dir(&managed).unwrap();
    let id = archive.keys.keys().next().unwrap();
    symlink(&victim, managed.join(id)).unwrap();
    assert!(archive.restore(Connections::default(), &managed).is_err());
    assert_eq!(fs::read_to_string(victim).unwrap(), "untouched");
}

#[test]
fn openssh_can_read_a_restored_passphrase_protected_key_without_the_original_file() {
    use std::process::Command;
    let folder = Folder::new();
    let source = folder.0.join("generated");
    let output = Command::new("ssh-keygen")
        .args(["-q", "-t", "ed25519", "-N", "test fixture only", "-f"])
        .arg(&source)
        .output()
        .expect("OpenSSH client is installed");
    assert!(output.status.success(), "ssh-keygen failed");
    let mut local = Connections::default();
    local
        .upsert(Host {
            address: "example.com".into(),
            identity: source.to_str().unwrap().into(),
            ..Host::default()
        })
        .unwrap();
    let expected = fs::read_to_string(source.with_extension("pub")).unwrap();
    let authority = folder.0.join("authority");
    let output = Command::new("ssh-keygen")
        .args(["-q", "-t", "ed25519", "-N", "", "-f"])
        .arg(&authority)
        .output()
        .unwrap();
    assert!(output.status.success());
    let output = Command::new("ssh-keygen")
        .args(["-q", "-s"])
        .arg(&authority)
        .args(["-I", "ely-sync-test", "-V", "+1h"])
        .arg(source.with_extension("pub"))
        .output()
        .unwrap();
    assert!(output.status.success());
    let certificate = folder.0.join("generated-cert.pub");
    let expected_certificate = fs::read(&certificate).unwrap();
    let archive = Archive::capture(&local).unwrap();
    fs::remove_file(source).unwrap();
    fs::remove_file(certificate).unwrap();
    let restored = archive
        .restore(Connections::default(), &folder.0.join("restored"))
        .unwrap();
    let output = Command::new("ssh-keygen")
        .args(["-y", "-P", "test fixture only", "-f"])
        .arg(&restored.hosts[0].identity)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "restored key is readable by OpenSSH"
    );
    let actual = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        actual.split_whitespace().take(2).collect::<Vec<_>>(),
        expected.split_whitespace().take(2).collect::<Vec<_>>()
    );
    let certificate = format!("{}-cert.pub", restored.hosts[0].identity);
    assert_eq!(fs::read(&certificate).unwrap(), expected_certificate);
    assert!(
        Command::new("ssh-keygen")
            .args(["-L", "-f", &certificate])
            .output()
            .unwrap()
            .status
            .success()
    );
}
