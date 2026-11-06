use std::collections::BTreeMap;

use anyhow::ensure;

use super::{Archive, PortableHost};

impl Archive {
    pub(in crate::ssh::sync) fn normalize(&mut self) -> anyhow::Result<()> {
        self.validate()?;
        self.hosts = std::mem::take(&mut self.hosts)
            .into_iter()
            .map(|mut host| {
                host.identify();
                host
            })
            .collect();
        let redundant: Vec<_> = self
            .hosts
            .iter()
            .filter(|host| {
                self.hosts
                    .iter()
                    .filter(|other| host.legacy_copy(other))
                    .count()
                    == 1
            })
            .map(PortableHost::id)
            .collect();
        self.deleted.extend(redundant);
        let exclusions = super::Connections {
            deleted_hosts: self.exclusions.clone(),
            ..Default::default()
        };
        for host in &self.hosts {
            if exclusions.is_deleted(&host.host(String::new()), host.key.as_deref()) {
                self.deleted.insert(host.id());
            }
        }
        self.hosts.retain(|host| !self.deleted.contains(&host.id()));
        self.prune();
        self.format = 2;
        self.snapshots.clear();
        self.validate()
    }

    fn prune(&mut self) {
        self.credentials
            .retain(|id, _| self.hosts.iter().any(|host| host.id() == *id));
        self.keys
            .retain(|id, _| self.hosts.iter().any(|host| host.key.as_ref() == Some(id)));
    }

    pub fn merge(&mut self, other: Self) -> anyhow::Result<()> {
        let mut merged = self.clone();
        merged.merge_records(other)?;
        *self = merged;
        Ok(())
    }

    fn merge_records(&mut self, mut other: Self) -> anyhow::Result<()> {
        self.normalize()?;
        other.normalize()?;
        self.deleted.extend(other.deleted);
        self.exclusions.extend(other.exclusions);
        let mut hosts: BTreeMap<_, _> = std::mem::take(&mut self.hosts)
            .into_iter()
            .map(|host| (host.id(), host))
            .collect();
        for incoming in other.hosts {
            let id = incoming.id();
            if let Some(existing) = hosts.get(&id) {
                let version = incoming.sync.version().cmp(&existing.sync.version());
                if version.is_eq() {
                    ensure!(
                        *existing == incoming,
                        "SSH sync: one record revision has conflicting contents"
                    );
                    if let (Some(left), Some(right)) =
                        (self.credentials.get(&id), other.credentials.get(&id))
                    {
                        ensure!(
                            left == right,
                            "SSH sync: credentials differ at the same revision; save the edited connection before synchronizing"
                        );
                    }
                }
                let same_auth = incoming.same_auth(existing);
                if version.is_gt() {
                    if !same_auth || other.credentials.contains_key(&id) {
                        self.credentials.remove(&id);
                    }
                    hosts.insert(id.clone(), incoming);
                } else if !version.is_eq() && (!same_auth || self.credentials.contains_key(&id)) {
                    other.credentials.remove(&id);
                }
            } else {
                hosts.insert(id.clone(), incoming);
            }
            if let Some(credential) = other.credentials.remove(&id) {
                self.credentials.insert(id, credential);
            }
        }
        self.hosts = hosts.into_values().collect();
        self.keys.extend(other.keys);
        self.prune();
        self.normalize()
    }
}
