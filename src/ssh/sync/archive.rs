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

mod host;
mod merge;
mod restore;
mod secrets;
pub(super) use host::PortableHost;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Archive {
    #[serde(default = "legacy_format")]
    pub format: u32,
    pub hosts: BTreeSet<PortableHost>,
    pub keys: BTreeMap<String, Key>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub credentials: BTreeMap<String, Credential>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub deleted: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub exclusions: BTreeSet<crate::ssh::deleted::Deletion>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub snapshots: BTreeSet<String>,
}

fn legacy_format() -> u32 {
    1
}

impl Default for Archive {
    fn default() -> Self {
        Self {
            format: 1,
            hosts: BTreeSet::new(),
            keys: BTreeMap::new(),
            credentials: BTreeMap::new(),
            deleted: BTreeSet::new(),
            exclusions: BTreeSet::new(),
            snapshots: BTreeSet::new(),
        }
    }
}

pub(crate) fn legacy_id(host: &Host) -> anyhow::Result<String> {
    let key = if host.identity.is_empty() {
        None
    } else {
        match Key::capture(Path::new(&host.identity)) {
            Ok(key) => Some(key.id()),
            Err(error) => {
                log::warn!(
                    "SSH sync: legacy key unavailable for {:?}; preserving its local identity until repaired: {error:#}",
                    host.label()
                );
                Some(format!("unavailable:{}", host.identity))
            }
        }
    };
    let mut portable = PortableHost::new(host, key);
    portable.sync = Default::default();
    Ok(portable.legacy_id())
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
        let mut archive = Self {
            format: 2,
            deleted: local.records.deleted.clone(),
            exclusions: local.deleted_hosts.clone(),
            ..Self::default()
        };
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
            let mut portable = PortableHost::new(host, key);
            portable.identify();
            archive.hosts.insert(portable);
        }
        archive.normalize()?;
        Ok(archive)
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            matches!(self.format, 1 | 2),
            "SSH sync: unsupported archive format; update MioTerm"
        );
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
        crate::ssh::records::State {
            clock: 0,
            deleted: self.deleted.clone(),
        }
        .validate()?;
        crate::ssh::deleted::validate(&self.exclusions)?;
        let mut ids = BTreeSet::new();
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
            host.sync.validate()?;
            ensure!(ids.insert(host.id()), "SSH sync: duplicate record identity");
            ensure!(
                self.format == 1 || !host.sync.is_empty(),
                "SSH sync: record identity is missing"
            );
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

    pub fn visible_counts(&self, local: &Connections) -> (usize, usize) {
        let hosts = self.visible_hosts(local);
        let keys: BTreeSet<_> = hosts.iter().filter_map(|host| host.key.as_ref()).collect();
        (hosts.len(), keys.len())
    }

    fn visible_hosts<'a>(&'a self, local: &Connections) -> Vec<&'a PortableHost> {
        self.hosts
            .iter()
            .filter(|host| {
                !self.deleted.contains(&host.id())
                    && !local.records.deleted.contains(&host.id())
                    && !local.is_deleted(&host.host(String::new()), host.key.as_deref())
            })
            .collect()
    }
}
