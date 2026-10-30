use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, Zeroizing};

use crate::ssh::{Host, auth, vault::Vault};

#[derive(PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub(super) struct Credential(pub String);

impl Drop for Credential {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl Credential {
    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            !self.0.is_empty() && self.0.len() <= 1023 && !self.0.contains(['\0', '\r', '\n']),
            "SSH sync: invalid saved credential"
        );
        Ok(())
    }
}

pub(super) trait Store {
    fn load(&self, host: &Host) -> anyhow::Result<Option<Zeroizing<String>>>;
    fn insert(&self, host: &Host, secret: &str) -> anyhow::Result<()>;
}

pub(super) struct VaultStore;

impl Store for VaultStore {
    fn load(&self, host: &Host) -> anyhow::Result<Option<Zeroizing<String>>> {
        auth::load(host)
    }

    fn insert(&self, host: &Host, secret: &str) -> anyhow::Result<()> {
        auth::import(host, secret)
    }
}

pub(super) fn token() -> anyhow::Result<Zeroizing<String>> {
    Vault::open()?.get("sync:gitee")?
        .context("Gitee sync: no saved token in the application vault; enter it or import system credentials")
}

pub(super) fn validate_token(value: &str) -> anyhow::Result<()> {
    ensure!(
        !value.is_empty()
            && value.len() <= 4096
            && value.bytes().all(|byte| byte.is_ascii_graphic()),
        "Gitee sync: enter a token without spaces or control characters"
    );
    Ok(())
}
