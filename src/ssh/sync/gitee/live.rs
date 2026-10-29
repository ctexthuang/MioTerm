use std::{
    cell::RefCell,
    fs,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context as _, ensure};

use super::*;
use crate::ssh::{
    Authentication, Connections, Host, auth,
    sync::{
        Remote, Request, WebDav, credential_tests::MemoryStore, credentials::Store,
        synchronize_with, tests::Folder,
    },
};

#[derive(Default)]
struct AppCredentials(RefCell<Vec<Host>>);

impl Store for AppCredentials {
    fn load(&self, host: &Host) -> anyhow::Result<Option<Zeroizing<String>>> {
        auth::load(host)
    }

    fn insert(&self, host: &Host, secret: &str) -> anyhow::Result<()> {
        ensure!(
            self.load(host)?.is_none(),
            "Gitee live test: fixture credential already exists"
        );
        auth::import(host, secret)?;
        self.0.borrow_mut().push(host.clone());
        Ok(())
    }
}

impl AppCredentials {
    fn cleanup(&mut self) -> anyhow::Result<()> {
        while let Some(host) = self.0.get_mut().last() {
            auth::forget(host)?;
            self.0.get_mut().pop();
        }
        Ok(())
    }
}

impl Drop for AppCredentials {
    fn drop(&mut self) {
        if let Err(error) = self.cleanup() {
            eprintln!("Gitee live test: cannot remove a temporary fixture credential: {error}");
        }
    }
}

struct Vault {
    client: Client,
    created: Vec<String>,
}

impl Vault {
    fn upload(&mut self, archive: &Archive, passphrase: &SecretString) -> anyhow::Result<()> {
        self.created
            .push(self.client.put(&crypto::encrypt(archive, passphrase)?)?);
        Ok(())
    }

    fn cleanup(&mut self) -> anyhow::Result<()> {
        while let Some(id) = self.created.last() {
            let response = self
                .client
                .agent
                .delete(format!("{}/gists/{id}", self.client.base))
                .header("Authorization", self.client.authorization())
                .call()
                .map_err(|_| anyhow::anyhow!("Gitee live test: cleanup request failed"))?;
            ensure!(
                matches!(response.status().as_u16(), 204 | 404),
                "Gitee live test: cleanup returned HTTP {}",
                response.status().as_u16()
            );
            self.created.pop();
        }
        Ok(())
    }
}

impl Drop for Vault {
    fn drop(&mut self) {
        if let Err(error) = self.cleanup() {
            eprintln!("{error}; remove the temporary test snapshots manually");
        }
    }
}

#[test]
#[ignore = "uses the saved Gitee token; creates and removes private fixture snapshots"]
fn saved_token_round_trips_private_keys_credentials_and_concurrent_snapshots() -> anyhow::Result<()>
{
    let token = SecretString::from(credentials::token()?.to_string());
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let settings = Gitee {
        vault: format!("mt{:012x}", stamp & 0xffffffffffff),
    };
    let passphrase = SecretString::from("635841".to_string());
    let mut vault = Vault {
        client: Client::new(&settings, &token)?,
        created: Vec::new(),
    };
    ensure!(
        vault.client.pull(&passphrase)?.is_none(),
        "Gitee live test: temporary vault already exists"
    );
    eprintln!("Gitee live test: authenticated listing passed");

    let folder = Folder::new();
    let identity = folder.0.join("live-key");
    let key_passphrase = "temporary live test key passphrase";
    let generated = Command::new("ssh-keygen")
        .args(["-q", "-t", "ed25519", "-N", key_passphrase, "-f"])
        .arg(&identity)
        .output()
        .context("Gitee live test: cannot run ssh-keygen")?;
    ensure!(
        generated.status.success(),
        "Gitee live test: key generation failed"
    );
    let public = fs::read_to_string(identity.with_extension("pub"))?;
    let mut local = Connections::default();
    let key_host = local.upsert(Host {
        address: format!("key-{}.invalid", settings.vault),
        identity: identity
            .to_str()
            .context("Gitee live test: non-UTF-8 test path")?
            .into(),
        auth: Authentication::PrivateKey,
        remember_secret: true,
        ..Host::default()
    })?;
    let password_host = local.upsert(Host {
        address: format!("password-{}.invalid", settings.vault),
        auth: Authentication::Password,
        remember_secret: true,
        ..Host::default()
    })?;
    let source = MemoryStore::default();
    source.insert(&key_host, key_passphrase)?;
    source.insert(&password_host, "temporary live test SSH password")?;
    let mut first = Archive::capture(&local)?;
    first.capture_credentials(&local, &source)?;
    vault.upload(&first, &passphrase)?;

    let mut branch = Connections::default();
    branch.upsert(Host {
        address: format!("branch-{}.invalid", settings.vault),
        ..Host::default()
    })?;
    vault.upload(&Archive::capture(&branch)?, &passphrase)?;
    eprintln!("Gitee live test: two independent private encrypted snapshots uploaded");
    let merged = vault
        .client
        .pull(&passphrase)?
        .context("Gitee live test: snapshots not found")?;
    ensure!(
        merged.hosts.len() == 3 && merged.keys.len() == 1 && merged.credentials.len() == 2,
        "Gitee live test: merged data counts differ"
    );
    ensure!(
        merged.snapshots.len() == 2,
        "Gitee live test: concurrent snapshot history differs"
    );
    vault.upload(&merged, &passphrase)?;
    fs::remove_file(&identity)?;

    let mut destination = AppCredentials::default();
    let restored = synchronize_with(
        Request {
            settings: WebDav::default(),
            gitee: Some(settings.clone()),
            include_credentials: true,
            remember_token: false,
            password: token.clone(),
            passphrase: passphrase.clone(),
            local: Connections::default(),
            key_folder: folder.0.join("new-machine"),
            restore_only: true,
        },
        Remote::Gitee(Client::new(&settings, &token)?),
        &destination,
    )?;
    ensure!(
        restored.hosts == 3 && restored.keys == 1 && restored.credentials == 2,
        "Gitee live test: restored data counts differ"
    );
    let host = restored
        .connections
        .hosts
        .iter()
        .find(|host| host.auth == Authentication::PrivateKey)
        .context("Gitee live test: restored key host missing")?;
    ensure!(
        host.remember_secret
            && destination.load(host)?.as_deref().map(String::as_str) == Some(key_passphrase),
        "Gitee live test: restored private-key passphrase differs"
    );
    let derived = Command::new("ssh-keygen")
        .args(["-y", "-P", key_passphrase, "-f"])
        .arg(&host.identity)
        .output()
        .context("Gitee live test: cannot inspect the restored key")?;
    ensure!(
        derived.status.success(),
        "Gitee live test: OpenSSH rejected the restored encrypted key"
    );
    let actual = String::from_utf8(derived.stdout)?;
    ensure!(
        actual
            .split_whitespace()
            .take(2)
            .eq(public.split_whitespace().take(2)),
        "Gitee live test: restored key has a different public identity"
    );
    let password = restored
        .connections
        .hosts
        .iter()
        .find(|host| host.auth == Authentication::Password)
        .context("Gitee live test: password host missing")?;
    ensure!(
        destination.load(password)?.as_deref().map(String::as_str)
            == Some("temporary live test SSH password"),
        "Gitee live test: application vault SSH password restoration differs"
    );
    ensure!(
        auth::import(host, "conflicting test fixture").is_err(),
        "Gitee live test: a conflicting local credential was overwritten"
    );
    ensure!(
        vault
            .client
            .pull(&age::x25519::Identity::generate().to_string())
            .is_err(),
        "Gitee live test: wrong encryption passphrase was accepted"
    );
    eprintln!(
        "Gitee live test: restore, credential recovery, OpenSSH key verification and wrong-passphrase rejection passed"
    );
    destination.cleanup()?;
    vault.cleanup()?;
    eprintln!(
        "Gitee live test: temporary private snapshots removed; existing user vaults were not modified"
    );
    Ok(())
}
