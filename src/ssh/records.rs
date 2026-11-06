use std::collections::BTreeSet;

use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};

use super::{Connections, Host};

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Record {
    pub id: String,
    pub revision: u64,
    pub change: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct State {
    pub clock: u64,
    pub deleted: BTreeSet<String>,
}

pub(crate) fn valid_id(id: &str) -> bool {
    id.len() == 64
        && id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn random_id() -> anyhow::Result<String> {
    let mut bytes = [0; 32];
    getrandom::fill(&mut bytes).context("SSH sync: cannot generate a record identity")?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

impl Record {
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }

    pub(crate) fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            self.is_empty()
                || (valid_id(&self.id)
                    && ((self.revision == 0 && self.change.is_empty())
                        || (self.revision > 0 && valid_id(&self.change)))),
            "SSH sync: invalid record identity or revision"
        );
        Ok(())
    }

    pub(crate) fn version(&self) -> (u64, &str) {
        (self.revision, &self.change)
    }
}

impl State {
    pub(crate) fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            self.deleted.len() <= 10000 && self.deleted.iter().all(|id| valid_id(id)),
            "SSH sync: invalid or excessive deleted record identities"
        );
        Ok(())
    }
}

impl Connections {
    pub(super) fn credential_edited(&mut self, host: &Host) -> anyhow::Result<()> {
        if !host.remember_secret || !host.auth.has_secret() {
            return Ok(());
        }
        self.migrate_records()?;
        let account = super::auth::account(host);
        let mut changed = 0;
        for index in 0..self.hosts.len() {
            let saved = &self.hosts[index];
            if saved.remember_secret && super::auth::account(saved) == account {
                let record = self.edited_record(&saved.clone())?;
                self.hosts[index].sync = record.into();
                changed += 1;
            }
        }
        log::info!("SSH sync: shared credential edit revised {changed} saved connections");
        Ok(())
    }

    pub(super) fn migrate_records(&mut self) -> anyhow::Result<bool> {
        let before = self.clone();
        for host in &mut self.hosts {
            if host.sync.is_empty() {
                host.sync.id = super::sync::legacy_id(host)?;
            }
            self.records.clock = self.records.clock.max(host.sync.revision);
        }
        let mut oldest = std::collections::BTreeMap::new();
        for host in &self.hosts {
            oldest
                .entry(host.sync.id.clone())
                .and_modify(|id: &mut u64| *id = (*id).min(host.id))
                .or_insert(host.id);
        }
        let mut removed = BTreeSet::new();
        self.hosts.retain(|host| {
            let keep = oldest.get(&host.sync.id) == Some(&host.id);
            if !keep {
                removed.insert(host.id);
                log::info!("SSH sync: removed identical legacy duplicate {}", host.id);
            }
            keep
        });
        self.recent
            .retain(|recent| !removed.contains(&recent.host.id));
        let redundant: Vec<_> = self
            .hosts
            .iter()
            .filter(|host| host.sync.revision == 0 && host.group.is_empty())
            .filter(|host| {
                let matches: Vec<_> = self
                    .hosts
                    .iter()
                    .filter(|other| legacy_copy(host, other))
                    .collect();
                matches.len() == 1 && matches[0].id < host.id
            })
            .map(|host| (host.id, host.sync.id.clone()))
            .collect();
        for (id, sync_id) in redundant {
            self.hosts.retain(|host| host.id != id);
            self.recent.retain(|recent| recent.host.id != id);
            self.records.deleted.insert(sync_id);
            log::info!("SSH sync: removed legacy ungrouped duplicate {id}");
        }
        for recent in &mut self.recent {
            if recent.host.sync.is_empty()
                && let Some(host) = self
                    .hosts
                    .iter()
                    .find(|host| host.id == recent.host.id && host.same_target(&recent.host))
            {
                recent.host.sync = host.sync.clone();
            }
        }
        Ok(*self != before)
    }

    pub(super) fn edited_record(&mut self, host: &Host) -> anyhow::Result<Record> {
        let id = self
            .hosts
            .iter()
            .find(|saved| saved.id == host.id && host.id != 0)
            .map(|saved| saved.sync.id.clone())
            .filter(|id| !id.is_empty())
            .map(Ok)
            .unwrap_or_else(random_id)?;
        self.records.clock = self
            .records
            .clock
            .max(
                self.hosts
                    .iter()
                    .map(|host| host.sync.revision)
                    .max()
                    .unwrap_or(0),
            )
            .checked_add(1)
            .context("SSH sync: record revision exhausted")?;
        Ok(Record {
            id,
            revision: self.records.clock,
            change: random_id()?,
        })
    }
}

fn legacy_copy(host: &Host, other: &Host) -> bool {
    other.sync.revision == 0
        && !host.name.is_empty()
        && !other.group.is_empty()
        && host.name == other.name
        && host.same_target(other)
}
