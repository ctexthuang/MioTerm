use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};

use super::{Connections, Host, keys::Key};

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Archive {
    pub hosts: BTreeSet<PortableHost>,
    pub keys: BTreeMap<String, Key>,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PortableHost {
    name: String,
    address: String,
    port: u16,
    user: String,
    group: String,
    key: Option<String>,
    #[serde(default)]
    auth: crate::ssh::Authentication,
}

impl PortableHost {
    fn new(host: &Host, key: Option<String>) -> Self {
        Self {
            name: host.name.clone(),
            address: host.address.clone(),
            port: host.port,
            user: host.user.clone(),
            group: host.group.clone(),
            key,
            auth: host.auth,
        }
    }

    fn host(&self, identity: String) -> Host {
        Host {
            id: 0,
            name: self.name.clone(),
            address: self.address.clone(),
            port: self.port,
            user: self.user.clone(),
            group: self.group.clone(),
            identity,
            auth: self.auth,
            remember_secret: false,
        }
    }

    fn same_fields(&self, host: &Host) -> bool {
        (
            &self.name,
            &self.address,
            self.port,
            &self.user,
            &self.group,
            self.auth,
        ) == (
            &host.name,
            &host.address,
            host.port,
            &host.user,
            &host.group,
            host.auth,
        )
    }
}

impl Archive {
    pub fn capture(local: &Connections) -> anyhow::Result<Self> {
        local.validate()?;
        let mut archive = Self::default();
        for host in &local.hosts {
            let key = if host.identity.is_empty() {
                None
            } else {
                let key = Key::capture(Path::new(&host.identity)).with_context(|| {
                    format!("WebDAV: cannot include the key for {}", host.label())
                })?;
                let id = key.id();
                archive.keys.insert(id.clone(), key);
                Some(id)
            };
            archive.hosts.insert(PortableHost::new(host, key));
        }
        archive.validate()?;
        Ok(archive)
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            self.hosts.len() <= 10000 && self.keys.len() <= 1000,
            "WebDAV: too many hosts or keys"
        );
        for (id, key) in &self.keys {
            key.validate()?;
            ensure!(
                *id == key.id(),
                "WebDAV: SSH key digest does not match its contents"
            );
            ensure!(
                self.hosts.iter().any(|host| host.key.as_ref() == Some(id)),
                "WebDAV: unreferenced SSH key"
            );
        }
        for host in &self.hosts {
            host.host(if host.key.is_some() {
                "/key".into()
            } else {
                String::new()
            })
            .validate()?;
            if let Some(key) = &host.key {
                ensure!(
                    self.keys.contains_key(key),
                    "WebDAV: a host references a missing private key"
                );
            }
        }
        Ok(())
    }

    pub fn merge(&mut self, other: Self) -> anyhow::Result<()> {
        other.validate()?;
        self.hosts.extend(other.hosts);
        for (id, key) in other.keys {
            self.keys.entry(id).or_insert(key);
        }
        self.validate()
    }

    pub fn restore(&self, mut local: Connections, folder: &Path) -> anyhow::Result<Connections> {
        self.validate()?;
        let mut installed = BTreeMap::new();
        for (id, key) in &self.keys {
            installed.insert(id.clone(), key.install(folder)?);
        }
        let mut existing = BTreeMap::new();
        for host in &local.hosts {
            if !host.identity.is_empty() && Path::new(&host.identity).is_file() {
                existing.insert(host.id, Key::capture(Path::new(&host.identity))?.id());
            }
        }
        for portable in &self.hosts {
            let identity = portable
                .key
                .as_ref()
                .map(|key| {
                    installed
                        .get(key)
                        .cloned()
                        .context("WebDAV: restored key is missing")
                })
                .transpose()?
                .unwrap_or_default();
            let same = local.hosts.iter_mut().find(|host| {
                portable.same_fields(host)
                    && match &portable.key {
                        None => host.identity.is_empty(),
                        Some(key) => {
                            existing.get(&host.id) == Some(key)
                                || (!host.identity.is_empty()
                                    && !Path::new(&host.identity).exists())
                        }
                    }
            });
            if let Some(host) = same {
                if !host.identity.is_empty() && !Path::new(&host.identity).exists() {
                    host.identity = identity;
                }
            } else {
                let host = local.upsert(portable.host(identity))?;
                if let Some(key) = &portable.key {
                    existing.insert(host.id, key.clone());
                }
            }
        }
        local.validate()?;
        Ok(local)
    }
}
