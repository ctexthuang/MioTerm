use std::collections::BTreeSet;

use crate::ssh::{Connections, Host, deleted::Deletion, records::State};

#[derive(PartialEq, Eq)]
pub(super) struct Changes {
    hosts: Vec<Host>,
    deleted: BTreeSet<Deletion>,
    records: State,
}

impl From<&Connections> for Changes {
    fn from(data: &Connections) -> Self {
        Self {
            hosts: data.hosts.clone(),
            deleted: data.deleted_hosts.clone(),
            records: data.records.clone(),
        }
    }
}

impl Changes {
    pub fn observe(&mut self, data: &Connections) -> bool {
        let next = Self::from(data);
        if *self == next {
            return false;
        }
        *self = next;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_connections_does_not_trigger_sync_but_edits_and_deletions_do() {
        let mut data = Connections::default();
        let host = data.upsert(Host::default()).unwrap();
        let mut changes = Changes::from(&data);
        data.opened(host.clone(), 123);
        assert!(!changes.observe(&data));
        data.upsert(Host {
            group: "Servers".into(),
            ..host.clone()
        })
        .unwrap();
        assert!(changes.observe(&data));
        assert!(!changes.observe(&data));
        data.remove(host.id).unwrap();
        assert!(changes.observe(&data));
        data.records.deleted.insert("a".repeat(64));
        assert!(changes.observe(&data));
    }

    #[test]
    fn imported_records_are_baselined_without_hiding_subsequent_local_edits() {
        let mut data = Connections::default();
        let mut changes = Changes::from(&data);
        let imported = data.upsert(Host::default()).unwrap();
        assert!(changes.observe(&data));
        changes = Changes::from(&data);
        assert!(!changes.observe(&data));
        data.upsert(Host {
            name: "Edited during sync".into(),
            ..imported
        })
        .unwrap();
        assert!(changes.observe(&data));
    }
}
