use age::secrecy::{ExposeSecret, SecretString};
use anyhow::{Context as _, ensure};
use sha2::{Digest as _, Sha256};

use super::{Provider, SyncSettings, WebDav, credentials, crypto};
use crate::ssh::vault::Vault;

#[derive(Clone, PartialEq, Eq)]
pub(super) struct Profile {
    pub webdav: WebDav,
    pub settings: SyncSettings,
}

#[derive(Default)]
pub(super) struct Secrets {
    pub password: SecretString,
    pub passphrase: SecretString,
}

pub(super) trait Store {
    fn get(&self, account: &str) -> anyhow::Result<Option<SecretString>>;
    fn put(&self, account: &str, value: &str) -> anyhow::Result<()>;
    fn remove(&self, account: &str) -> anyhow::Result<()>;
}

pub(super) struct VaultStore(pub Vault);

impl Store for VaultStore {
    fn get(&self, account: &str) -> anyhow::Result<Option<SecretString>> {
        Ok(self
            .0
            .get(&format!("sync:{account}"))?
            .map(|value| SecretString::from(value.to_string())))
    }

    fn put(&self, account: &str, value: &str) -> anyhow::Result<()> {
        self.0.put(&format!("sync:{account}"), value)
    }

    fn remove(&self, account: &str) -> anyhow::Result<()> {
        self.0.remove(&format!("sync:{account}"))
    }
}

impl Profile {
    fn password_account(&self) -> anyhow::Result<String> {
        Ok(format!(
            "webdav:{:x}",
            Sha256::digest(serde_json::to_vec(&(
                self.webdav.endpoint()?.as_str(),
                &self.webdav.username
            ))?)
        ))
    }

    fn passphrase_account(&self, token: &SecretString) -> anyhow::Result<String> {
        let location = match self.settings.provider {
            Provider::Gitee => {
                ensure!(
                    !token.expose_secret().is_empty(),
                    "SSH sync: enter the Gitee token to locate this vault's remembered encryption passphrase"
                );
                format!(
                    "gitee:{}:{:x}",
                    self.settings.gitee.vault,
                    Sha256::digest(token.expose_secret())
                )
            }
            Provider::WebDav => self.password_account()?,
        };
        Ok(format!("archive:{:x}", Sha256::digest(location)))
    }

    pub fn resolve(
        &self,
        mut secrets: Secrets,
        synchronize: bool,
        store: &impl Store,
    ) -> anyhow::Result<Secrets> {
        self.webdav.validate()?;
        self.settings.validate(&self.webdav)?;
        let settings = &self.settings;
        let remember_password = if settings.provider == Provider::Gitee {
            settings.remember_token
        } else {
            settings.remember_password
        };
        if secrets.password.expose_secret().is_empty() && remember_password {
            let account = if settings.provider == Provider::Gitee {
                "gitee".into()
            } else {
                self.password_account()?
            };
            secrets.password = store.get(&account)?.context(
                "SSH sync: no access credential is remembered for this server; enter it and save again",
            )?;
        }
        if settings.provider == Provider::Gitee && !secrets.password.expose_secret().is_empty() {
            credentials::validate_token(secrets.password.expose_secret())?;
        }
        if secrets.passphrase.expose_secret().is_empty() && settings.remember_passphrase {
            secrets.passphrase = store
                .get(&self.passphrase_account(&secrets.password)?)?
                .context("SSH sync: no encryption passphrase is remembered for this vault; enter it and save again")?;
        }
        if synchronize || !secrets.passphrase.expose_secret().is_empty() {
            crypto::validate_passphrase(&secrets.passphrase)?;
        }
        if synchronize && settings.provider == Provider::WebDav {
            self.webdav.endpoint()?;
        }
        Ok(secrets)
    }

    pub fn remember(
        &self,
        previous: &Self,
        secrets: &Secrets,
        store: &impl Store,
    ) -> anyhow::Result<()> {
        let before = &previous.settings;
        let next = &self.settings;
        let old_token = if before.remember_passphrase && before.provider == Provider::Gitee {
            store.get("gitee")?
        } else {
            None
        };
        if next.remember_passphrase {
            crypto::validate_passphrase(&secrets.passphrase)?;
            store.put(
                &self.passphrase_account(&secrets.password)?,
                secrets.passphrase.expose_secret(),
            )?;
        } else if before.remember_passphrase {
            let token = old_token.as_ref().unwrap_or(&secrets.password);
            store.remove(&previous.passphrase_account(token)?)?;
        }
        if next.provider == Provider::Gitee && next.remember_token {
            credentials::validate_token(secrets.password.expose_secret())?;
            store.put("gitee", secrets.password.expose_secret())?;
        } else if before.remember_token && !next.remember_token {
            store.remove("gitee")?;
        }
        if next.provider == Provider::WebDav && next.remember_password {
            store.put(&self.password_account()?, secrets.password.expose_secret())?;
        } else if before.remember_password && !next.remember_password {
            store.remove(&previous.password_account()?)?;
        }
        Ok(())
    }
}

#[cfg(test)]
pub(super) mod tests;
