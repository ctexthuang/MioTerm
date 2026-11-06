use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use super::{Host, Key};
use crate::ssh::Record;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::ssh::sync) struct PortableHost {
    pub(super) name: String,
    pub(super) address: String,
    pub(super) port: u16,
    pub(super) user: String,
    pub(super) group: String,
    pub(super) key: Option<String>,
    #[serde(default)]
    pub(super) auth: crate::ssh::Authentication,
    #[serde(default, skip_serializing_if = "Record::is_empty")]
    pub(super) sync: Record,
}

impl PortableHost {
    pub(super) fn legacy_id(&self) -> String {
        let mut legacy = self.clone();
        legacy.sync = Record::default();
        format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&legacy).expect("portable SSH host"))
        )
    }

    pub(super) fn id(&self) -> String {
        if self.sync.is_empty() {
            self.legacy_id()
        } else {
            self.sync.id.clone()
        }
    }

    pub(super) fn identify(&mut self) {
        if self.sync.is_empty() {
            self.sync.id = self.legacy_id();
        }
    }

    pub(super) fn capture(host: &Host) -> anyhow::Result<Self> {
        let key = if host.identity.is_empty() {
            None
        } else {
            Some(Key::capture(Path::new(&host.identity))?.id())
        };
        let mut portable = Self::new(host, key);
        portable.identify();
        Ok(portable)
    }

    pub(super) fn new(host: &Host, key: Option<String>) -> Self {
        Self {
            name: host.name.clone(),
            address: host.address.clone(),
            port: host.port,
            user: host.user.clone(),
            group: host.group.clone(),
            key,
            auth: host.auth,
            sync: (*host.sync).clone(),
        }
    }

    pub(super) fn host(&self, identity: String) -> Host {
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
            keepalive: Default::default(),
            sync: self.sync.clone().into(),
        }
    }

    pub(super) fn same_fields(&self, host: &Host) -> bool {
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

    pub(super) fn same_auth(&self, other: &Self) -> bool {
        (&self.address, self.port, &self.user, self.auth, &self.key)
            == (
                &other.address,
                other.port,
                &other.user,
                other.auth,
                &other.key,
            )
    }

    pub(super) fn legacy_copy(&self, other: &Self) -> bool {
        self.sync.revision == 0
            && other.sync.revision == 0
            && self.group.is_empty()
            && !other.group.is_empty()
            && !self.name.is_empty()
            && self.name == other.name
            && self.same_auth(other)
    }
}
