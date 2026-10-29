use anyhow::ensure;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    #[default]
    WebDav,
    Gitee,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Gitee {
    pub vault: String,
}

impl Default for Gitee {
    fn default() -> Self {
        Self {
            vault: "main".into(),
        }
    }
}

impl Gitee {
    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            !self.vault.is_empty()
                && self.vault.len() <= 16
                && self
                    .vault
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte)),
            "Gitee sync: vault name must contain 1-16 ASCII letters, digits, hyphens or underscores"
        );
        Ok(())
    }

    pub(super) fn description(&self) -> String {
        format!("MioTerm Sync {}", self.vault)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SyncSettings {
    pub provider: Provider,
    pub gitee: Gitee,
    pub include_credentials: bool,
    pub remember_token: bool,
    pub remember_password: bool,
    pub remember_passphrase: bool,
    pub auto_sync: bool,
}

impl SyncSettings {
    pub fn validate(&self, webdav: &super::WebDav) -> anyhow::Result<()> {
        self.gitee.validate()?;
        if self.auto_sync {
            ensure!(
                self.remember_passphrase,
                "SSH sync: automatic synchronization requires a remembered encryption passphrase"
            );
            match self.provider {
                Provider::Gitee => ensure!(
                    self.remember_token,
                    "SSH sync: automatic synchronization requires a remembered Gitee token"
                ),
                Provider::WebDav => {
                    webdav.endpoint()?;
                    ensure!(
                        self.remember_password,
                        "SSH sync: automatic synchronization requires a remembered WebDAV password"
                    );
                }
            }
        }
        Ok(())
    }
}
