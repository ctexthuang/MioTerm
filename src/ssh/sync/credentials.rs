use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, Zeroizing};

use crate::ssh::{Host, auth};

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

pub(super) struct SystemStore;

impl Store for SystemStore {
    fn load(&self, host: &Host) -> anyhow::Result<Option<Zeroizing<String>>> {
        auth::load(host)
    }

    fn insert(&self, host: &Host, secret: &str) -> anyhow::Result<()> {
        auth::import(host, secret)
    }
}

fn token_entry() -> anyhow::Result<keyring::Entry> {
    keyring::Entry::new("io.github.ctexthuang.mioterm.sync", "gitee")
        .context("Gitee sync: cannot open the system credential store")
}

pub(super) fn token() -> anyhow::Result<Zeroizing<String>> {
    token_entry()?
        .get_password()
        .map(Zeroizing::new)
        .context("Gitee sync: cannot read a saved token; enter it again")
}

pub(super) fn save_token(value: &str, remember: bool) -> anyhow::Result<()> {
    let entry = token_entry()?;
    if remember {
        if !value.is_empty() {
            validate_token(value)?;
            entry
                .set_password(value)
                .context("Gitee sync: cannot save the token")?;
        }
    } else {
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(error) => return Err(error).context("Gitee sync: cannot remove the saved token"),
        }
    }
    Ok(())
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
