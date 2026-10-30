use std::collections::BTreeSet;

use anyhow::ensure;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use super::{Connections, Host, keys::Key};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Deletion {
    profile: String,
    identity: Identity,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Identity {
    None,
    Digest(String),
    Unavailable,
}

fn profile(host: &Host) -> String {
    let fields = serde_json::to_vec(&(
        &host.name,
        &host.address,
        host.port,
        &host.user,
        &host.group,
        host.auth,
    ))
    .expect("serializable SSH profile");
    format!("{:x}", Sha256::digest(fields))
}

fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

impl Deletion {
    pub(super) fn capture(host: &Host) -> Self {
        let identity = if host.identity.is_empty() {
            Identity::None
        } else {
            match Key::capture(std::path::Path::new(&host.identity)) {
                Ok(key) => Identity::Digest(key.id()),
                Err(error) => {
                    log::warn!(
                        "ssh: cannot fingerprint the key while deleting {:?}; matching key-backed sync entries will stay hidden: {error:#}",
                        host.label()
                    );
                    Identity::Unavailable
                }
            }
        };
        Self {
            profile: profile(host),
            identity,
        }
    }

    fn matches(&self, profile: &str, key: Option<&str>) -> bool {
        self.profile == profile
            && match &self.identity {
                Identity::None => key.is_none(),
                Identity::Digest(id) => key == Some(id.as_str()),
                Identity::Unavailable => key.is_some(),
            }
    }

    fn overlaps(&self, other: &Self) -> bool {
        self.profile == other.profile
            && (self.identity == other.identity
                || matches!(
                    (&self.identity, &other.identity),
                    (Identity::Unavailable, Identity::Digest(_))
                        | (Identity::Digest(_), Identity::Unavailable)
                ))
    }
}

pub(super) fn validate(deleted: &BTreeSet<Deletion>) -> anyhow::Result<()> {
    ensure!(deleted.len() <= 10000, "ssh: too many deleted host records");
    for entry in deleted {
        ensure!(
            digest(&entry.profile),
            "ssh: invalid deleted host fingerprint"
        );
        if let Identity::Digest(key) = &entry.identity {
            ensure!(digest(key), "ssh: invalid deleted host key digest");
        }
    }
    Ok(())
}

impl Connections {
    pub(super) fn is_deleted(&self, host: &Host, key: Option<&str>) -> bool {
        if self.deleted_hosts.is_empty() {
            return false;
        }
        let profile = profile(host);
        self.deleted_hosts
            .iter()
            .any(|entry| entry.matches(&profile, key))
    }

    pub(super) fn allow_host(&mut self, host: &Host) {
        if self.deleted_hosts.is_empty() {
            return;
        }
        let profile = profile(host);
        if self
            .deleted_hosts
            .iter()
            .any(|entry| entry.profile == profile)
        {
            let added = Deletion::capture(host);
            let before = self.deleted_hosts.len();
            self.deleted_hosts.retain(|entry| !entry.overlaps(&added));
            if self.deleted_hosts.len() != before {
                log::info!(
                    "ssh: explicitly saved host {:?} is allowed in sync again",
                    host.label()
                );
            }
        }
    }
}
