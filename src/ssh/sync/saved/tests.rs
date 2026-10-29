use std::{cell::RefCell, collections::BTreeMap};

use super::*;

#[derive(Default)]
pub(in crate::ssh::sync) struct Memory(RefCell<BTreeMap<String, SecretString>>);

impl Store for Memory {
    fn get(&self, account: &str) -> anyhow::Result<Option<SecretString>> {
        Ok(self.0.borrow().get(account).cloned())
    }

    fn put(&self, account: &str, value: &str) -> anyhow::Result<()> {
        self.0
            .borrow_mut()
            .insert(account.into(), value.to_owned().into());
        Ok(())
    }

    fn remove(&self, account: &str) -> anyhow::Result<()> {
        self.0.borrow_mut().remove(account);
        Ok(())
    }
}

fn gitee() -> Profile {
    Profile {
        webdav: WebDav::default(),
        settings: SyncSettings {
            provider: Provider::Gitee,
            remember_token: true,
            remember_passphrase: true,
            auto_sync: true,
            ..SyncSettings::default()
        },
    }
}

fn secrets() -> Secrets {
    Secrets {
        password: "fixture-token".to_owned().into(),
        passphrase: "635841".to_owned().into(),
    }
}

#[test]
fn six_character_passphrases_survive_save_and_restart_without_plaintext_settings() {
    let store = Memory::default();
    let profile = gitee();
    let resolved = profile.resolve(secrets(), false, &store).unwrap();
    profile.remember(&profile, &resolved, &store).unwrap();
    let json = serde_json::to_string(&profile.settings).unwrap();
    assert!(!json.contains("635841") && !json.contains("fixture-token"));
    let restarted = Profile {
        settings: serde_json::from_str(&json).unwrap(),
        ..profile
    };
    for _ in 0..2 {
        let loaded = restarted.resolve(Secrets::default(), true, &store).unwrap();
        assert_eq!(loaded.passphrase.expose_secret(), "635841");
        assert_eq!(loaded.password.expose_secret(), "fixture-token");
    }
}

#[test]
fn encryption_passphrases_are_scoped_to_vault_and_gitee_account_token() {
    let store = Memory::default();
    let profile = gitee();
    let input = secrets();
    profile.remember(&profile, &input, &store).unwrap();
    let mut other = profile.clone();
    other.settings.gitee.vault = "another".into();
    assert!(other.resolve(Secrets::default(), true, &store).is_err());
    store.put("gitee", "different-account-token").unwrap();
    assert!(profile.resolve(Secrets::default(), true, &store).is_err());
    let mut replacement = secrets();
    replacement.password = "different-account-token".to_owned().into();
    profile.remember(&profile, &replacement, &store).unwrap();
    assert!(profile.resolve(Secrets::default(), true, &store).is_ok());
}

#[test]
fn webdav_secrets_are_scoped_to_endpoint_and_username() {
    let store = Memory::default();
    let profile = Profile {
        webdav: WebDav {
            url: "https://dav.example/folder/".into(),
            username: "alice".into(),
        },
        settings: SyncSettings {
            remember_password: true,
            remember_passphrase: true,
            auto_sync: true,
            ..SyncSettings::default()
        },
    };
    profile.remember(&profile, &secrets(), &store).unwrap();
    let loaded = profile.resolve(Secrets::default(), true, &store).unwrap();
    assert_eq!(loaded.passphrase.expose_secret(), "635841");
    for (url, username) in [
        ("https://dav.example/other", "alice"),
        ("https://dav.example/folder/", "bob"),
    ] {
        let other = Profile {
            webdav: WebDav {
                url: url.into(),
                username: username.into(),
            },
            ..profile.clone()
        };
        assert!(other.resolve(Secrets::default(), true, &store).is_err());
    }
}

#[test]
fn turning_remembrance_off_removes_stored_values_and_requires_input_again() {
    let store = Memory::default();
    let before = gitee();
    before.remember(&before, &secrets(), &store).unwrap();
    let mut next = before.clone();
    next.settings.auto_sync = false;
    next.settings.remember_passphrase = false;
    next.settings.remember_token = false;
    next.remember(&before, &Secrets::default(), &store).unwrap();
    assert!(store.0.borrow().is_empty());
    assert!(next.resolve(Secrets::default(), true, &store).is_err());
    assert!(next.resolve(Secrets::default(), false, &store).is_ok());
}

#[test]
fn auto_sync_requires_remembered_secrets_and_never_falls_back_to_empty_encryption() {
    let store = Memory::default();
    let mut profile = gitee();
    assert!(profile.resolve(Secrets::default(), true, &store).is_err());
    profile.settings.remember_passphrase = false;
    assert!(profile.settings.validate(&profile.webdav).is_err());
    profile.settings.remember_passphrase = true;
    profile.settings.remember_token = false;
    assert!(profile.settings.validate(&profile.webdav).is_err());
    profile.settings.auto_sync = false;
    assert!(profile.settings.validate(&profile.webdav).is_ok());
    let mut short = secrets();
    short.passphrase = "12345".to_owned().into();
    assert!(profile.resolve(short, false, &store).is_err());
}

#[test]
fn file_vault_remembers_secrets_across_profile_and_storage_reconstruction() {
    let folder = crate::ssh::vault::tests::Folder::new();
    let path = folder.0.join("vault");
    let store = VaultStore(Vault::at(path.clone()));
    let profile = Profile {
        webdav: WebDav {
            url: "https://fixture.invalid/dav/".into(),
            username: "fixture".into(),
        },
        settings: SyncSettings {
            remember_password: true,
            remember_passphrase: true,
            auto_sync: true,
            ..SyncSettings::default()
        },
    };
    let accounts = [
        profile.password_account().unwrap(),
        profile
            .passphrase_account(&SecretString::default())
            .unwrap(),
    ];
    assert!(
        accounts
            .iter()
            .all(|account| store.get(account).unwrap().is_none())
    );
    let previous = Profile {
        settings: SyncSettings::default(),
        ..profile.clone()
    };
    profile.remember(&previous, &secrets(), &store).unwrap();
    let restarted = VaultStore(Vault::at(path));
    let profile = Profile {
        settings: serde_json::from_str(&serde_json::to_string(&profile.settings).unwrap()).unwrap(),
        ..profile
    };
    let loaded = profile
        .resolve(Secrets::default(), true, &restarted)
        .unwrap();
    assert_eq!(loaded.password.expose_secret(), "fixture-token");
    assert_eq!(loaded.passphrase.expose_secret(), "635841");
    previous
        .remember(&profile, &Secrets::default(), &restarted)
        .unwrap();
    assert!(
        accounts
            .iter()
            .all(|account| restarted.get(account).unwrap().is_none())
    );
}
