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
    assert!(!saved.sync.remember_password);
    assert!(!saved.sync.remember_passphrase);
    assert!(!saved.sync.auto_sync);
    assert_eq!(saved.webdav.username, "alice");
    let text = serde_json::to_string(&saved.sync).unwrap();
    let fields: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert!(fields.get("password").is_none() && fields.get("passphrase").is_none());
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
fn passwords_require_opt_in_and_restore_to_the_credential_store_not_connection_files() {
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
fn cloud_archive_restores_key_bytes_and_passwords_into_an_independent_device_vault() {
    use crate::ssh::vault::Vault;
    struct Files(Vault);
    impl Store for Files {
        fn load(&self, host: &Host) -> anyhow::Result<Option<Zeroizing<String>>> {
            self.0.get(&format!("ssh:{}", auth::account(host)))
        }
        fn insert(&self, host: &Host, secret: &str) -> anyhow::Result<()> {
            self.0
                .insert(&format!("ssh:{}", auth::account(host)), secret)
        }
    }
    let folder = tests::Folder::new();
    let identity = folder.0.join("original-macos-key");
    std::fs::write(&identity, "-----BEGIN OPENSSH PRIVATE KEY-----\nSYNTHETIC FIXTURE\n-----END OPENSSH PRIVATE KEY-----\n").unwrap();
    let mut local = password_host();
    local
        .upsert(Host {
            address: "key-fixture.example".into(),
            auth: Authentication::PrivateKey,
            identity: identity.display().to_string(),
            remember_secret: true,
            ..Host::default()
        })
        .unwrap();
    let source = Files(Vault::at(folder.0.join("source-app-data")));
    source.insert(&local.hosts[0], "fixture-password").unwrap();
    source
        .insert(&local.hosts[1], "fixture-key-passphrase")
        .unwrap();
    let mut archive = Archive::capture(&local).unwrap();
    archive.capture_credentials(&local, &source).unwrap();
    let plain = serde_json::to_string(&archive).unwrap();
    assert!(!plain.contains(&identity.display().to_string()));
    assert!(!plain.contains("local.key") && !plain.contains("sync:gitee"));
    let phrase = SecretString::from("635841".to_owned());
    let bytes = crypto::encrypt(&archive, &phrase).unwrap();
    std::fs::remove_file(&identity).unwrap();
    let received = crypto::decrypt(&bytes, &phrase).unwrap();
    let key_folder = folder.0.join("destination-home/.ssh/elyterminal-sync");
    let mut restored = received
        .restore(Connections::default(), &key_folder)
        .unwrap();
    let destination = Files(Vault::at(folder.0.join("destination-app-data")));
    assert_eq!(
        received
            .restore_credentials(&mut restored, &destination)
            .unwrap(),
        2
    );
    for host in &restored.hosts {
        let expected = if host.auth == Authentication::PrivateKey {
            "fixture-key-passphrase"
        } else {
            "fixture-password"
        };
        assert_eq!(destination.load(host).unwrap().unwrap().as_str(), expected);
        assert!(host.remember_secret);
        if host.auth == Authentication::PrivateKey {
            assert!(std::path::Path::new(&host.identity).starts_with(&key_folder));
            assert!(std::path::Path::new(&host.identity).is_file());
        }
    }
    assert_ne!(
        std::fs::read(folder.0.join("source-app-data/local.key")).unwrap(),
        std::fs::read(folder.0.join("destination-app-data/local.key")).unwrap()
    );
}

#[test]
fn missing_passwords_invalid_records_and_conflicting_credentials_fail_without_overwriting() {
    let local = password_host();
    let store = MemoryStore::default();
    let mut archive = Archive::capture(&local).unwrap();
    let error = archive.capture_credentials(&local, &store).unwrap_err();
    assert!(error.is::<credentials::MissingPasswords>());
    assert!(error.to_string().contains("credential-fixture.example"));
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
fn missing_password_is_reported_before_contacting_the_provider() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let client = webdav::Client::at(
        &format!("http://{}/archive", listener.local_addr().unwrap()),
        "fixture",
        &SecretString::from("fixture-token".to_owned()),
    )
    .unwrap();
    let request = Request {
        settings: WebDav::default(),
        gitee: None,
        include_credentials: true,
        remember_token: false,
        password: SecretString::from("fixture-token".to_owned()),
        passphrase: SecretString::from("635841".to_owned()),
        local: password_host(),
        key_folder: std::env::temp_dir(),
        restore_only: false,
    };
    let error = synchronize_with(request, Remote::WebDav(client), &MemoryStore::default())
        .err()
        .expect("missing password must fail");
    assert!(error.is::<credentials::MissingPasswords>());
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
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
