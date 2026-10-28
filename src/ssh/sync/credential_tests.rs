use std::{cell::RefCell, collections::BTreeMap};

use age::secrecy::SecretString;
use zeroize::Zeroizing;

use super::{credentials::Store, *};
use crate::ssh::{Authentication, auth};

#[derive(Default)]
pub(super) struct MemoryStore(pub RefCell<BTreeMap<String, String>>);

impl Store for MemoryStore {
    fn load(&self, host: &Host) -> anyhow::Result<Option<Zeroizing<String>>> {
        Ok(self
            .0
            .borrow()
            .get(&auth::account(host))
            .cloned()
            .map(Zeroizing::new))
    }
    fn insert(&self, host: &Host, secret: &str) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.0.borrow().contains_key(&auth::account(host)),
            "fixture: no overwrite"
        );
        self.0
            .borrow_mut()
            .insert(auth::account(host), secret.into());
        Ok(())
    }
}

pub(super) fn password_host() -> Connections {
    let mut local = Connections::default();
    local
        .upsert(Host {
            address: "credential-fixture.example".into(),
            auth: Authentication::Password,
            remember_secret: true,
            ..Host::default()
        })
        .unwrap();
    local
}

#[test]
fn settings_keep_webdav_and_never_serialize_tokens_or_encryption_passwords() {
    let saved: crate::settings::Settings =
        serde_json::from_str(r#"{"webdav":{"url":"https://example.com/dav","username":"alice"}}"#)
            .unwrap();
    assert_eq!(saved.sync.provider, Provider::WebDav);
    assert!(!saved.sync.include_credentials);
    assert!(!saved.sync.remember_token);
    assert_eq!(saved.webdav.username, "alice");
    let text = serde_json::to_string(&saved.sync).unwrap();
    assert!(!text.contains("password") && !text.contains("passphrase"));
    for field in ["token", "password", "passphrase"] {
        assert!(
            serde_json::from_value::<SyncSettings>(serde_json::json!({field: "not allowed"}))
                .is_err()
        );
    }
    for vault in [
        "",
        "../other",
        "a/b",
        "https://host",
        "has space",
        "more-than-sixteen-characters",
    ] {
        assert!(
            Gitee {
                vault: vault.into()
            }
            .validate()
            .is_err()
        );
    }
    for token in ["", "has space", "line\n", "header\r\n", "\0"] {
        assert!(credentials::validate_token(token).is_err());
    }
    assert!(
        Gitee {
            vault: "work-vault_2".into()
        }
        .validate()
        .is_ok()
    );
}

#[test]
fn passwords_require_opt_in_and_restore_to_the_system_store_not_connection_files() {
    let local = password_host();
    let source = MemoryStore::default();
    source
        .insert(&local.hosts[0], "fixture password only")
        .unwrap();
    let mut archive = Archive::capture(&local).unwrap();
    assert!(archive.credentials.is_empty());
    archive.capture_credentials(&local, &source).unwrap();
    let phrase = SecretString::from("fixture encryption passphrase".to_string());
    let encrypted = crypto::encrypt(&archive, &phrase).unwrap();
    assert!(
        !encrypted
            .windows(b"fixture password only".len())
            .any(|bytes| bytes == b"fixture password only")
    );
    let archive = crypto::decrypt(&encrypted, &phrase).unwrap();
    let mut target = archive
        .restore(Connections::default(), &std::env::temp_dir())
        .unwrap();
    assert!(!target.hosts[0].remember_secret);
    let destination = MemoryStore::default();
    assert_eq!(
        archive
            .restore_credentials(&mut target, &destination)
            .unwrap(),
        1
    );
    assert_eq!(
        destination
            .load(&target.hosts[0])
            .unwrap()
            .unwrap()
            .as_str(),
        "fixture password only"
    );
    assert!(target.hosts[0].remember_secret);
    assert!(
        !serde_json::to_string(&target)
            .unwrap()
            .contains("fixture password only")
    );
}

#[test]
fn missing_passwords_invalid_records_and_conflicting_credentials_fail_without_overwriting() {
    let local = password_host();
    let store = MemoryStore::default();
    let mut archive = Archive::capture(&local).unwrap();
    assert!(archive.capture_credentials(&local, &store).is_err());
    store.insert(&local.hosts[0], "first fixture").unwrap();
    archive.capture_credentials(&local, &store).unwrap();
    let different = MemoryStore::default();
    different.insert(&local.hosts[0], "second fixture").unwrap();
    let mut other = Archive::capture(&local).unwrap();
    other.capture_credentials(&local, &different).unwrap();
    assert!(archive.merge(other).is_err());
    let mut target = local.clone();
    assert!(
        archive
            .restore_credentials(&mut target, &different)
            .is_err()
    );
    assert_eq!(
        different.load(&target.hosts[0]).unwrap().unwrap().as_str(),
        "second fixture"
    );
    let mut value = serde_json::to_value(&archive).unwrap();
    value["credentials"] = serde_json::json!({"unknown": "fixture"});
    assert!(
        serde_json::from_value::<Archive>(value)
            .unwrap()
            .validate()
            .is_err()
    );
    for invalid in ["", "line\n", "\0"] {
        assert!(credentials::Credential(invalid.into()).validate().is_err());
    }
}

#[test]
fn restoring_credentials_updates_matching_recent_connections_without_syncing_history() {
    let mut local = password_host();
    local.hosts[0].remember_secret = false;
    local.opened(local.hosts[0].clone(), 1);
    let mut imported = local.clone();
    imported.hosts[0].remember_secret = true;
    let restored = Restored {
        before: local.hosts.clone(),
        before_next_id: local.next_id,
        connections: imported,
        hosts: 1,
        keys: 0,
        credentials: 1,
    };
    restored.apply(&mut local).unwrap();
    assert_eq!(local.recent.len(), 1);
    assert!(local.recent[0].host.remember_secret);
}
