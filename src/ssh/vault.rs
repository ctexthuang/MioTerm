use std::{collections::BTreeMap, path::PathBuf};

use anyhow::ensure;
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, Zeroizing};

mod disk;
pub(super) mod migration;
#[cfg(test)]
pub(super) mod tests;

const LIMIT: usize = 8 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct Secret(String);

impl Drop for Secret {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Contents {
    entries: BTreeMap<String, Secret>,
}

pub(crate) struct Vault {
    folder: PathBuf,
}

impl Vault {
    #[cfg(test)]
    pub(super) fn at(folder: PathBuf) -> Self {
        Self { folder }
    }

    pub fn open() -> anyhow::Result<Self> {
        Ok(Self {
            folder: Self::folder()?,
        })
    }

    pub fn folder() -> anyhow::Result<PathBuf> {
        Ok(crate::settings::folder()?.join("credentials"))
    }

    pub fn get(&self, account: &str) -> anyhow::Result<Option<Zeroizing<String>>> {
        self.read(|contents| {
            contents
                .entries
                .get(account)
                .map(|value| Zeroizing::new(value.0.clone()))
        })
    }

    pub fn put(&self, account: &str, value: &str) -> anyhow::Result<()> {
        self.edit(|contents| {
            contents
                .entries
                .insert(account.into(), Secret(value.into()));
            Ok(())
        })?;
        log::info!("Credential vault: encrypted credential saved");
        Ok(())
    }

    pub fn remove(&self, account: &str) -> anyhow::Result<()> {
        self.edit(|contents| {
            contents.entries.remove(account);
            Ok(())
        })?;
        log::info!("Credential vault: saved credential removed");
        Ok(())
    }

    pub fn insert(&self, account: &str, value: &str) -> anyhow::Result<()> {
        self.insert_many(&[(account.into(), Zeroizing::new(value.into()))])
    }

    fn insert_many(&self, entries: &[(String, Zeroizing<String>)]) -> anyhow::Result<()> {
        self.edit(|contents| {
            for (account, value) in entries {
                if let Some(existing) = contents.entries.get(account) {
                    ensure!(
                        existing.0 == **value,
                        "Credential vault: local credential changed; it was not overwritten"
                    );
                }
                contents
                    .entries
                    .insert(account.clone(), Secret(value.to_string()));
            }
            Ok(())
        })?;
        log::info!("Credential vault: encrypted credentials imported");
        Ok(())
    }
}
