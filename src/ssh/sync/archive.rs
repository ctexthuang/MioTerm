use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};

use super::{
    Connections, Host,
    credentials::{Credential, Store},
    keys::Key,
};

use sha2::{Digest as _, Sha256};

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Archive {
    pub hosts: BTreeSet<PortableHost>,
    pub keys: BTreeMap<String, Key>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub credentials: BTreeMap<String, Credential>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub snapshots: BTreeSet<String>,
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
    fn id(&self) -> String {
        format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(self).expect("portable SSH host"))
        )
    }

    fn capture(host: &Host) -> anyhow::Result<Self> {
        let key = if host.identity.is_empty() {
            None
        } else {
            Some(Key::capture(Path::new(&host.identity))?.id())
        };
        Ok(Self::new(host, key))
    }
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
    pub fn capture_available(
        local: &Connections,
        store: &impl Store,
        include_credentials: bool,
    ) -> anyhow::Result<(Self, Vec<String>)> {
        local.validate()?;
        let mut included = local.clone();
        included.hosts.clear();
        let mut warnings = Vec::new();
        for host in &local.hosts {
            if !host.identity.is_empty()
                && !Path::new(&host.identity).try_exists().with_context(|| {
                    format!("SSH sync: cannot inspect the key for {}", host.label())
                })?
            {
                let warning = format!(
                    "Skipping local export for {:?}: its selected private key file is missing.",
                    host.label()
                );
                log::warn!("SSH sync: {warning}");
                warnings.push(warning);
            } else {
                included.hosts.push(host.clone());
            }
        }
        let mut archive = Self::capture(&included)?;
        if include_credentials {
            warnings.extend(archive.capture_credentials(&included, store)?);
        }
        Ok((archive, warnings))
    }

    pub fn capture(local: &Connections) -> anyhow::Result<Self> {
        local.validate()?;
        let mut archive = Self::default();
        for host in &local.hosts {
            let key = if host.identity.is_empty() {
                None
            } else {
                let key = Key::capture(Path::new(&host.identity)).with_context(|| {
                    format!("SSH sync: cannot include the key for {}", host.label())
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
            "SSH sync: too many hosts or keys"
        );
        ensure!(
            self.snapshots.len() <= 10000,
            "SSH sync: snapshot history is too large"
        );
        for snapshot in &self.snapshots {
            super::gitee::validate_id(snapshot)?;
        }
        ensure!(
            self.credentials.len() <= self.hosts.len(),
            "SSH sync: too many credentials"
        );
        let credential_hosts: BTreeMap<_, _> = self
            .hosts
            .iter()
            .map(|host| (host.id(), host.auth.has_secret()))
            .collect();
        for (id, credential) in &self.credentials {
            ensure!(
                credential_hosts.get(id) == Some(&true),
                "SSH sync: credential does not belong to a password or private-key host"
            );
            credential.validate()?;
        }
        for (id, key) in &self.keys {
            key.validate()?;
            ensure!(
                *id == key.id(),
                "SSH sync: SSH key digest does not match its contents"
            );
            ensure!(
                self.hosts.iter().any(|host| host.key.as_ref() == Some(id)),
                "SSH sync: unreferenced SSH key"
            );
        }
        for host in &self.hosts {
            host.host(if host.key.is_some() {
                if cfg!(windows) { r"C:\key" } else { "/key" }.into()
            } else {
                String::new()
            })
            .validate()?;
            if let Some(key) = &host.key {
                ensure!(
                    self.keys.contains_key(key),
                    "SSH sync: a host references a missing private key"
                );
            }
        }
        Ok(())
    }

    pub fn merge(&mut self, other: Self) -> anyhow::Result<()> {
        other.validate()?;
        for (id, credential) in other.credentials {
            if let Some(existing) = self.credentials.get(&id) {
                ensure!(
                    existing == &credential,
                    "SSH sync: credentials differ between devices; resolve them before synchronizing"
                );
            } else {
                self.credentials.insert(id, credential);
            }
        }
        self.snapshots.extend(other.snapshots);
        self.hosts.extend(other.hosts);
        for (id, key) in other.keys {
            self.keys.entry(id).or_insert(key);
        }
        self.validate()
    }

    pub fn capture_credentials(
        &mut self,
        local: &Connections,
        store: &impl Store,
    ) -> anyhow::Result<Vec<String>> {
        let mut warnings = Vec::new();
        for host in &local.hosts {
            if !self.hosts.iter().any(|portable| portable.same_fields(host)) {
                continue;
            }
            if host.remember_secret && host.auth.has_secret() {
                match store.load(host)? {
                    Some(secret) => {
                        self.credentials.insert(
                            PortableHost::capture(host)?.id(),
                            Credential(secret.to_string()),
                        );
                    }
                    None if host.auth == crate::ssh::Authentication::Password => {
                        let warning = format!(
                            "No saved SSH password for {:?}; exporting connection details only.",
                            host.label()
                        );
                        log::warn!("SSH sync: {warning}");
                        warnings.push(warning);
                    }
                    None => {}
                }
            }
        }
        self.validate()?;
        Ok(warnings)
    }

    pub fn restore_credentials(
        &self,
        local: &mut Connections,
        store: &impl Store,
    ) -> anyhow::Result<usize> {
        let mut pending = Vec::new();
        for host in &local.hosts {
            if !self.hosts.iter().any(|portable| portable.same_fields(host)) {
                continue;
            }
            if !host.identity.is_empty()
                && !Path::new(&host.identity).try_exists().with_context(|| {
                    format!("SSH sync: cannot inspect the key for {}", host.label())
                })?
            {
                continue;
            }
            let portable = PortableHost::capture(host)?;
            if local.is_deleted(host, portable.key.as_deref()) {
                continue;
            }
            if let Some(credential) = self.credentials.get(&portable.id()) {
                let existing = store.load(host)?;
                ensure!(
                    existing
                        .as_deref()
                        .map(|secret| secret.as_str())
                        .is_none_or(|secret| secret == credential.0),
                    "SSH sync: a local credential differs from the archive; it was not overwritten"
                );
                pending.push((host.id, credential, existing.is_none()));
            }
        }
        for (id, credential, missing) in &pending {
            let host = local
                .hosts
                .iter_mut()
                .find(|host| host.id == *id)
                .expect("restored SSH host");
            host.remember_secret = true;
            if *missing {
                store.insert(host, &credential.0)?;
            }
        }
        Ok(pending.len())
    }

    pub fn restore(&self, mut local: Connections, folder: &Path) -> anyhow::Result<Connections> {
        self.validate()?;
        let hosts = self.visible_hosts(&local);
        let needed: BTreeSet<_> = hosts.iter().filter_map(|host| host.key.clone()).collect();
        let mut installed = BTreeMap::new();
        for (id, key) in self.keys.iter().filter(|(id, _)| needed.contains(*id)) {
            installed.insert(id.clone(), key.install(folder)?);
        }
        let mut existing = BTreeMap::new();
        for host in &local.hosts {
            if !host.identity.is_empty() && Path::new(&host.identity).is_file() {
                existing.insert(host.id, Key::capture(Path::new(&host.identity))?.id());
            }
        }
        for portable in hosts {
            let identity = portable
                .key
                .as_ref()
                .map(|key| {
                    installed
                        .get(key)
                        .cloned()
                        .context("SSH sync: restored key is missing")
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
                let host = local.merge_host(portable.host(identity))?;
                if let Some(key) = &portable.key {
                    existing.insert(host.id, key.clone());
                }
            }
        }
        local.validate()?;
        Ok(local)
    }

    pub fn visible_counts(&self, local: &Connections) -> (usize, usize) {
        let hosts = self.visible_hosts(local);
        let keys: BTreeSet<_> = hosts.iter().filter_map(|host| host.key.as_ref()).collect();
        (hosts.len(), keys.len())
    }

    fn visible_hosts<'a>(&'a self, local: &Connections) -> Vec<&'a PortableHost> {
        self.hosts
            .iter()
            .filter(|host| !local.is_deleted(&host.host(String::new()), host.key.as_deref()))
            .collect()
    }
}
