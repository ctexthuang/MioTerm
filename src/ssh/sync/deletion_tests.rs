use super::{
    credential_tests::{MemoryStore, password_host},
    credentials::Store,
    *,
};
use crate::ssh::{Authentication, store};

#[test]
fn synchronizing_never_reimports_a_host_deleted_locally() {
    let mut local = password_host();
    let archive = Archive::capture(&local).unwrap();
    local.remove(local.hosts[0].id).unwrap();
    let restored = archive.restore(local, &std::env::temp_dir()).unwrap();
    assert!(
        restored.hosts.is_empty(),
        "cloud merge resurrected a deleted host"
    );
}

#[test]
fn local_deletions_survive_storage_reload_and_repeated_cloud_merges() {
    let folder = tests::Folder::new();
    let path = folder.0.join("connections.json");
    let mut local = password_host();
    let archive = Archive::capture(&local).unwrap();
    local.remove(local.hosts[0].id).unwrap();
    store::save(&path, &local).unwrap();
    let mut restored = store::load(&path).unwrap();
    assert_eq!(restored, local);
    for _ in 0..3 {
        restored = archive.restore(restored, &folder.0.join("keys")).unwrap();
        assert!(restored.hosts.is_empty());
        assert_eq!(restored.deleted_hosts, local.deleted_hosts);
    }
    let fresh_device = archive.restore(Connections::default(), &folder.0).unwrap();
    assert_eq!(
        fresh_device.hosts.len(),
        1,
        "local deletion must not erase another device's host"
    );
}

#[test]
fn an_explicitly_readded_host_is_allowed_but_sync_imports_never_clear_deletions() {
    let mut local = password_host();
    let archive = Archive::capture(&local).unwrap();
    let removed = local.remove(local.hosts[0].id).unwrap();
    let mut restored = archive.restore(local, &std::env::temp_dir()).unwrap();
    assert_eq!(restored.deleted_hosts.len(), 1);
    let added = restored.upsert(Host { id: 0, ..removed }).unwrap();
    assert!(restored.deleted_hosts.is_empty());
    restored = archive.restore(restored, &std::env::temp_dir()).unwrap();
    assert_eq!(restored.hosts, vec![added]);
}

#[test]
fn deleting_a_key_variant_does_not_hide_a_different_key_with_the_same_profile() {
    let folder = tests::Folder::new();
    let mut local = folder.local("first-key");
    let mut cloud = Archive::capture(&local).unwrap();
    let mut other = folder.local("other-key");
    other.hosts[0].name = local.hosts[0].name.clone();
    cloud.merge(Archive::capture(&other).unwrap()).unwrap();
    local.remove(local.hosts[0].id).unwrap();
    let journal = local.deleted_hosts.clone();
    let mut restored = cloud.restore(local, &folder.0.join("keys")).unwrap();
    for _ in 0..2 {
        assert_eq!(restored.hosts.len(), 1);
        assert_eq!(restored.deleted_hosts, journal);
        assert_eq!(
            std::fs::read(&restored.hosts[0].identity).unwrap(),
            std::fs::read(&other.hosts[0].identity).unwrap()
        );
        restored = cloud.restore(restored, &folder.0.join("keys")).unwrap();
    }
    restored.upsert(restored.hosts[0].clone()).unwrap();
    assert_eq!(
        restored.deleted_hosts, journal,
        "editing another key must not undo this deletion"
    );
}

#[test]
fn unavailable_keys_can_still_be_deleted_and_deleted_keys_are_not_reinstalled() {
    let folder = tests::Folder::new();
    let mut local = folder.local("removed-key");
    let archive = Archive::capture(&local).unwrap();
    std::fs::remove_file(&local.hosts[0].identity).unwrap();
    local.remove(local.hosts[0].id).unwrap();
    let keys = folder.0.join("must-not-be-installed");
    let restored = archive.restore(local, &keys).unwrap();
    assert!(restored.hosts.is_empty());
    assert!(!keys.exists());
}

#[test]
fn normal_sync_retains_the_cloud_backup_without_restoring_deleted_hosts_or_passwords() {
    let folder = tests::Folder::new();
    let mut local = password_host();
    let removed = local.hosts[0].clone();
    local
        .upsert(Host {
            id: 0,
            address: "kept.fixture.invalid".into(),
            ..removed.clone()
        })
        .unwrap();
    let source = MemoryStore::default();
    source.insert(&removed, "deleted fixture password").unwrap();
    source
        .insert(&local.hosts[1], "kept fixture password")
        .unwrap();
    let mut cloud = Archive::capture(&local).unwrap();
    cloud.capture_credentials(&local, &source).unwrap();
    local.remove(removed.id).unwrap();
    let destination = MemoryStore::default();
    let (restored, uploaded) =
        partial_tests::upload(local.clone(), Some(cloud), &destination, &folder.0);
    assert_eq!(
        (restored.hosts, restored.keys, restored.credentials),
        (1, 0, 1)
    );
    assert_eq!(restored.connections.hosts, local.hosts);
    assert_eq!(restored.connections.deleted_hosts, local.deleted_hosts);
    assert!(destination.load(&removed).unwrap().is_none());
    assert_eq!((uploaded.hosts.len(), uploaded.credentials.len()), (2, 2));
    let json = serde_json::to_string(&restored.connections).unwrap();
    assert!(!json.contains("fixture password"));
    let restored_again = uploaded.restore(restored.connections, &folder.0).unwrap();
    assert_eq!(restored_again.hosts.len(), 1);
}

#[test]
fn a_deletion_made_during_sync_cannot_be_overwritten_by_its_result() {
    let mut local = password_host();
    let result = Restored {
        before: local.hosts.clone(),
        before_next_id: local.next_id,
        before_deleted: local.deleted_hosts.clone(),
        connections: local.clone(),
        hosts: 1,
        keys: 0,
        credentials: 0,
        warnings: Vec::new(),
    };
    local.remove(local.hosts[0].id).unwrap();
    let deleted = local.clone();
    assert!(result.apply(&mut local).is_err());
    assert_eq!(local, deleted);
}

#[test]
fn old_connection_files_load_and_invalid_deletion_records_are_rejected() {
    let mut local: Connections =
        serde_json::from_str(r#"{"next_id":1,"hosts":[],"recent":[]}"#).unwrap();
    assert!(local.deleted_hosts.is_empty());
    local
        .upsert(Host {
            auth: Authentication::Password,
            ..Host::default()
        })
        .unwrap();
    local.remove(local.hosts[0].id).unwrap();
    let mut value = serde_json::to_value(&local).unwrap();
    value["deleted_hosts"][0]["profile"] = serde_json::json!("not-a-digest");
    let invalid: Connections = serde_json::from_value(value).unwrap();
    assert!(invalid.validate().is_err());
}
