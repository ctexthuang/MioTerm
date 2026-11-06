use super::{
    credential_tests::{MemoryStore, password_host},
    credentials::Store,
    tests::Folder,
    *,
};
use crate::ssh::{Authentication, store};

fn legacy(local: &Connections) -> Archive {
    let mut value = serde_json::to_value(Archive::capture(local).unwrap()).unwrap();
    value["format"] = serde_json::json!(1);
    for host in value["hosts"].as_array_mut().unwrap() {
        host.as_object_mut().unwrap().remove("sync");
    }
    serde_json::from_value(value).unwrap()
}

#[test]
fn group_name_and_destination_edits_replace_one_record_and_preserve_local_ids() {
    let folder = Folder::new();
    let mut first = password_host();
    let mut second = Archive::capture(&first)
        .unwrap()
        .restore(
            Connections {
                next_id: 40,
                ..Connections::default()
            },
            &folder.0,
        )
        .unwrap();
    let original_id = first.hosts[0].sync.id.clone();
    let local_id = second.hosts[0].id;
    assert_ne!(local_id, first.hosts[0].id);
    second.hosts[0].keepalive.interval = 85;
    for (name, group, address) in [
        ("Debian", "zytc", "one.example"),
        ("New name", "prod", "two.example"),
        ("New name", "", "two.example"),
    ] {
        first
            .upsert(Host {
                name: name.into(),
                group: group.into(),
                address: address.into(),
                ..first.hosts[0].clone()
            })
            .unwrap();
        let (restored, cloud) = partial_tests::upload(
            second,
            Some(Archive::capture(&first).unwrap()),
            &MemoryStore::default(),
            &folder.0,
        );
        second = restored.connections;
        assert_eq!(cloud.hosts.len(), 1);
        assert_eq!(second.hosts.len(), 1);
        let host = &second.hosts[0];
        assert_eq!(
            (host.id, host.sync.id.as_str()),
            (local_id, original_id.as_str())
        );
        assert_eq!(
            (
                host.name.as_str(),
                host.group.as_str(),
                host.address.as_str()
            ),
            (name, group, address)
        );
        assert_eq!(host.keepalive.interval, 85);
    }
}

#[test]
fn deletion_propagates_to_an_offline_device_and_never_reappears_after_reload() {
    let folder = Folder::new();
    let mut first = password_host();
    let stale = first.clone();
    let mut second = first.clone();
    first.remove(first.hosts[0].id).unwrap();
    let mut cloud = Archive::capture(&first).unwrap();
    for _ in 0..3 {
        let (result, uploaded) =
            partial_tests::upload(second, Some(cloud), &MemoryStore::default(), &folder.0);
        second = result.connections;
        cloud = uploaded;
        assert!(second.hosts.is_empty());
        assert!(cloud.hosts.is_empty());
        assert_eq!(cloud.deleted.len(), 1);
        let path = folder.0.join("connections.json");
        store::save(&path, &second).unwrap();
        second = store::load(&path).unwrap();
    }
    cloud.merge(Archive::capture(&stale).unwrap()).unwrap();
    assert!(cloud.hosts.is_empty());
    assert!(cloud.restore(stale, &folder.0).unwrap().hosts.is_empty());
    assert!(
        cloud
            .restore(Connections::default(), &folder.0)
            .unwrap()
            .hosts
            .is_empty()
    );
}

#[test]
fn deletion_wins_over_an_offline_edit_and_readding_creates_a_new_identity() {
    let folder = Folder::new();
    let mut first = password_host();
    let mut offline = first.clone();
    offline
        .upsert(Host {
            group: "offline edit".into(),
            ..offline.hosts[0].clone()
        })
        .unwrap();
    let removed = first.remove(first.hosts[0].id).unwrap();
    let mut cloud = Archive::capture(&offline).unwrap();
    cloud.merge(Archive::capture(&first).unwrap()).unwrap();
    assert!(cloud.hosts.is_empty());
    let added = first
        .upsert(Host {
            id: 0,
            ..removed.clone()
        })
        .unwrap();
    assert_ne!(removed.sync.id, added.sync.id);
    cloud.merge(Archive::capture(&first).unwrap()).unwrap();
    let restored = cloud.restore(offline, &folder.0).unwrap();
    assert_eq!(restored.hosts.len(), 1);
    assert_eq!(restored.hosts[0].sync, added.sync);
}

#[test]
fn concurrent_edits_converge_in_either_merge_order_and_later_edits_advance() {
    let folder = Folder::new();
    let mut left = password_host();
    let mut right = left.clone();
    left.upsert(Host {
        group: "left".into(),
        ..left.hosts[0].clone()
    })
    .unwrap();
    right
        .upsert(Host {
            group: "right".into(),
            ..right.hosts[0].clone()
        })
        .unwrap();
    let mut a = Archive::capture(&left).unwrap();
    a.merge(Archive::capture(&right).unwrap()).unwrap();
    let mut b = Archive::capture(&right).unwrap();
    b.merge(Archive::capture(&left).unwrap()).unwrap();
    assert_eq!(
        serde_json::to_value(&a).unwrap(),
        serde_json::to_value(&b).unwrap()
    );
    let mut restored = a.restore(left, &folder.0).unwrap();
    let revision = restored.hosts[0].sync.revision;
    restored
        .upsert(Host {
            group: "after sync".into(),
            ..restored.hosts[0].clone()
        })
        .unwrap();
    assert!(restored.hosts[0].sync.revision > revision);
    b.merge(Archive::capture(&restored).unwrap()).unwrap();
    assert_eq!(
        b.restore(right, &folder.0).unwrap().hosts[0].group,
        "after sync"
    );
}

#[test]
fn password_changes_follow_the_newer_record_without_losing_connection_identity() {
    let folder = Folder::new();
    let mut source = password_host();
    let target = source.clone();
    let saved = MemoryStore::default();
    saved.insert(&target.hosts[0], "old password").unwrap();
    let updated = MemoryStore::default();
    updated.insert(&source.hosts[0], "new password").unwrap();
    source.upsert(source.hosts[0].clone()).unwrap();
    let mut cloud = Archive::capture(&source).unwrap();
    cloud.capture_credentials(&source, &updated).unwrap();
    let (result, cloud) = partial_tests::upload(target, Some(cloud), &saved, &folder.0);
    assert_eq!(result.connections.hosts[0].sync.id, source.hosts[0].sync.id);
    assert_eq!(
        saved
            .load(&result.connections.hosts[0])
            .unwrap()
            .unwrap()
            .as_str(),
        "new password"
    );
    assert_eq!(cloud.credentials.len(), 1);
}

#[test]
fn changing_a_private_key_updates_the_same_record_and_prunes_the_old_key() {
    let folder = Folder::new();
    let mut source = folder.local("old-key");
    source
        .upsert(Host {
            auth: Authentication::PrivateKey,
            ..source.hosts[0].clone()
        })
        .unwrap();
    let target = source.clone();
    let other = folder.local("new-key");
    source
        .upsert(Host {
            identity: other.hosts[0].identity.clone(),
            ..source.hosts[0].clone()
        })
        .unwrap();
    let mut cloud = Archive::capture(&target).unwrap();
    cloud.merge(Archive::capture(&source).unwrap()).unwrap();
    assert_eq!((cloud.hosts.len(), cloud.keys.len()), (1, 1));
    let restored = cloud.restore(target, &folder.0.join("restored")).unwrap();
    assert_eq!(restored.hosts[0].sync.id, source.hosts[0].sync.id);
    assert_eq!(
        std::fs::read(&restored.hosts[0].identity).unwrap(),
        std::fs::read(&source.hosts[0].identity).unwrap()
    );
}

#[test]
fn legacy_group_duplicates_are_removed_but_distinct_keys_and_named_profiles_survive() {
    let folder = Folder::new();
    let mut local = folder.local("key");
    local.hosts[0].name = "debian-test".into();
    local.hosts[0].group = "zytc".into();
    local
        .upsert(Host {
            id: 0,
            group: String::new(),
            ..local.hosts[0].clone()
        })
        .unwrap();
    let mut value = serde_json::to_value(&local).unwrap();
    for host in value["hosts"].as_array_mut().unwrap() {
        host.as_object_mut().unwrap().remove("sync");
    }
    let path = folder.0.join("connections.json");
    std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    let migrated = store::load(&path).unwrap();
    assert_eq!(migrated.hosts.len(), 1);
    assert_eq!(migrated.hosts[0].group, "zytc");
    assert_eq!(migrated.records.deleted.len(), 1);
    assert_eq!(store::load(&path).unwrap(), migrated);
    let mut cloud = legacy(&local);
    cloud.normalize().unwrap();
    assert_eq!(cloud.hosts.len(), 1);
    let mut variants = migrated.clone();
    variants
        .upsert(Host {
            id: 0,
            name: "separate profile".into(),
            ..variants.hosts[0].clone()
        })
        .unwrap();
    let other = folder.local("different-key");
    variants
        .upsert(Host {
            id: 0,
            identity: other.hosts[0].identity.clone(),
            ..variants.hosts[0].clone()
        })
        .unwrap();
    cloud.merge(Archive::capture(&variants).unwrap()).unwrap();
    assert_eq!((cloud.hosts.len(), cloud.keys.len()), (3, 2));
}

#[test]
fn legacy_credentials_and_deletions_migrate_without_restoring_old_records() {
    let folder = Folder::new();
    let local = password_host();
    let mut old = legacy(&local);
    let id = archive::legacy_id(&local.hosts[0]).unwrap();
    old.credentials
        .insert(id, credentials::Credential("legacy secret".into()));
    old.normalize().unwrap();
    let mut restored = old.restore(Connections::default(), &folder.0).unwrap();
    let store = MemoryStore::default();
    assert_eq!(old.restore_credentials(&mut restored, &store).unwrap(), 1);
    assert_eq!(
        store.load(&restored.hosts[0]).unwrap().unwrap().as_str(),
        "legacy secret"
    );
    let mut deletion = Connections::default();
    deletion
        .deleted_hosts
        .insert(crate::ssh::deleted::Deletion::capture(&local.hosts[0]));
    old.merge(Archive::capture(&deletion).unwrap()).unwrap();
    assert!(old.hosts.is_empty() && old.credentials.is_empty());
    assert!(old.restore(restored, &folder.0).unwrap().hosts.is_empty());
}

#[test]
fn failed_merge_preserves_the_original_archive_and_rejects_duplicate_record_ids() {
    let local = password_host();
    let mut one = Archive::capture(&local).unwrap();
    let original = serde_json::to_value(&one).unwrap();
    let mut invalid = local.clone();
    invalid.hosts[0].group = "changed without saving".into();
    assert!(one.merge(Archive::capture(&invalid).unwrap()).is_err());
    assert_eq!(serde_json::to_value(&one).unwrap(), original);
    let mut value = original;
    let mut duplicate = value["hosts"][0].clone();
    duplicate["group"] = serde_json::json!("duplicate identity");
    value["hosts"].as_array_mut().unwrap().push(duplicate);
    assert!(
        serde_json::from_value::<Archive>(value)
            .unwrap()
            .validate()
            .is_err()
    );
}

#[test]
fn identical_legacy_duplicates_keep_one_local_id_and_do_not_create_a_deletion() {
    let folder = Folder::new();
    let mut local = password_host();
    local
        .upsert(Host {
            id: 0,
            ..local.hosts[0].clone()
        })
        .unwrap();
    let mut value = serde_json::to_value(&local).unwrap();
    for host in value["hosts"].as_array_mut().unwrap() {
        host.as_object_mut().unwrap().remove("sync");
    }
    let path = folder.0.join("connections.json");
    std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    let migrated = store::load(&path).unwrap();
    assert_eq!(migrated.hosts.len(), 1);
    assert_eq!(migrated.hosts[0].id, local.hosts[0].id);
    assert!(migrated.records.deleted.is_empty());
    assert_eq!(store::load(&path).unwrap(), migrated);
}

#[test]
fn editing_a_migrated_record_before_its_first_sync_preserves_legacy_identity() {
    let folder = Folder::new();
    let mut cloud = legacy(&password_host());
    cloud.normalize().unwrap();
    let mut migrated = cloud.restore(Connections::default(), &folder.0).unwrap();
    let id = migrated.hosts[0].sync.id.clone();
    migrated
        .upsert(Host {
            group: "new group".into(),
            ..migrated.hosts[0].clone()
        })
        .unwrap();
    cloud.merge(Archive::capture(&migrated).unwrap()).unwrap();
    let restored = cloud.restore(Connections::default(), &folder.0).unwrap();
    assert_eq!(restored.hosts.len(), 1);
    assert_eq!(restored.hosts[0].group, "new group");
    assert_eq!(restored.hosts[0].sync.id, id);
}

#[test]
fn damaged_legacy_keys_do_not_prevent_loading_or_deleting_the_saved_profile() {
    let folder = Folder::new();
    let local = folder.local("damaged-legacy-key");
    let mut value = serde_json::to_value(&local).unwrap();
    value["hosts"][0].as_object_mut().unwrap().remove("sync");
    std::fs::write(&local.hosts[0].identity, "not a key").unwrap();
    let path = folder.0.join("connections.json");
    std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    let mut migrated = store::load(&path).unwrap();
    assert_eq!(migrated.hosts.len(), 1);
    assert!(Archive::capture(&migrated).is_err());
    migrated.remove(migrated.hosts[0].id).unwrap();
    assert!(migrated.hosts.is_empty());
}

#[test]
fn applying_remote_deletions_removes_matching_legacy_recent_entries() {
    let folder = Folder::new();
    let mut local = password_host();
    let mut recent = local.hosts[0].clone();
    recent.sync = Box::default();
    local.opened(recent, 1);
    let mut cloud = local.clone();
    cloud.remove(cloud.hosts[0].id).unwrap();
    let (restored, _) = partial_tests::upload(
        local.clone(),
        Some(Archive::capture(&cloud).unwrap()),
        &MemoryStore::default(),
        &folder.0,
    );
    restored.apply(&mut local).unwrap();
    assert!(local.hosts.is_empty() && local.recent.is_empty());
}
