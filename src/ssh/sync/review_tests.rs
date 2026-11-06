use super::{
    credential_tests::{MemoryStore, password_host},
    credentials::Store,
    tests::Folder,
    *,
};
use crate::ssh::{Authentication, store};

#[test]
fn deleting_a_legacy_host_with_an_unavailable_key_retires_its_cloud_identity() {
    for damaged in [false, true] {
        let folder = Folder::new();
        let original = folder.local("legacy-key");
        let mut cloud = serde_json::to_value(Archive::capture(&original).unwrap()).unwrap();
        cloud["format"] = 1.into();
        cloud["hosts"][0].as_object_mut().unwrap().remove("sync");
        let mut cloud: Archive = serde_json::from_value(cloud).unwrap();
        let mut local = serde_json::to_value(&original).unwrap();
        local["hosts"][0].as_object_mut().unwrap().remove("sync");
        let path = folder.0.join("connections.json");
        std::fs::write(&path, serde_json::to_vec(&local).unwrap()).unwrap();
        if damaged {
            std::fs::write(&original.hosts[0].identity, "damaged test key").unwrap();
        } else {
            std::fs::remove_file(&original.hosts[0].identity).unwrap();
        }
        let mut local = store::load(&path).unwrap();
        local.remove(local.hosts[0].id).unwrap();
        cloud.merge(Archive::capture(&local).unwrap()).unwrap();
        assert!(
            cloud.hosts.is_empty(),
            "deleted legacy host returned from cloud"
        );
        assert!(cloud.restore(local, &folder.0).unwrap().hosts.is_empty());
    }
}

#[test]
fn applying_a_remote_authentication_change_updates_recent_connections() {
    let folder = Folder::new();
    let mut local = password_host();
    local.opened(local.hosts[0].clone(), 1);
    let mut remote = local.clone();
    let edited = remote
        .upsert(Host {
            auth: Authentication::Agent,
            remember_secret: false,
            ..remote.hosts[0].clone()
        })
        .unwrap();
    let (restored, _) = partial_tests::upload(
        local.clone(),
        Some(Archive::capture(&remote).unwrap()),
        &MemoryStore::default(),
        &folder.0,
    );
    restored.apply(&mut local).unwrap();
    assert_eq!(local.recent[0].host.auth, Authentication::Agent);
    assert_eq!(local.recent[0].host.sync, edited.sync);
    assert!(!local.recent[0].host.remember_secret);
}

#[test]
fn changing_a_saved_destination_does_not_replace_a_historical_destinations_key() {
    let folder = Folder::new();
    let mut local = folder.local("old-destination-key");
    let historical = local.hosts[0].clone();
    local.opened(historical.clone(), 1);
    let mut remote = local.clone();
    let replacement = folder.local("new-destination-key");
    remote
        .upsert(Host {
            address: "new-destination.example".into(),
            identity: replacement.hosts[0].identity.clone(),
            ..remote.hosts[0].clone()
        })
        .unwrap();
    let (restored, _) = partial_tests::upload(
        local.clone(),
        Some(Archive::capture(&remote).unwrap()),
        &MemoryStore::default(),
        &folder.0,
    );
    local.opened(historical.clone(), 2);
    restored.apply(&mut local).unwrap();
    assert_eq!(local.hosts[0].address, "new-destination.example");
    assert_eq!(local.recent[0].host, historical);
}

#[test]
fn editing_a_shared_password_can_sync_all_profiles_that_use_it() {
    let folder = Folder::new();
    let mut target = password_host();
    target
        .upsert(Host {
            id: 0,
            name: "Alias".into(),
            ..target.hosts[0].clone()
        })
        .unwrap();
    let saved = MemoryStore::default();
    saved
        .insert(&target.hosts[0], "old fixture password")
        .unwrap();
    let updated = MemoryStore::default();
    updated
        .insert(&target.hosts[0], "new fixture password")
        .unwrap();
    let mut source = target.clone();
    source.credential_edited(&source.hosts[0].clone()).unwrap();
    source.upsert(source.hosts[0].clone()).unwrap();
    let mut cloud = Archive::capture(&source).unwrap();
    cloud.capture_credentials(&source, &updated).unwrap();
    let (restored, _) = partial_tests::upload(target, Some(cloud), &saved, &folder.0);
    for host in &restored.connections.hosts {
        assert_eq!(
            saved.load(host).unwrap().unwrap().as_str(),
            "new fixture password"
        );
    }
}

#[test]
fn metadata_edits_leave_peer_versions_alone_but_quick_password_edits_revise_them() {
    let mut local = password_host();
    let peer = local
        .upsert(Host {
            id: 0,
            name: "Alias".into(),
            ..local.hosts[0].clone()
        })
        .unwrap();
    local
        .upsert(Host {
            group: "Renamed".into(),
            ..local.hosts[0].clone()
        })
        .unwrap();
    assert_eq!(local.hosts[1].sync, peer.sync);
    let before = local.clone();
    local
        .credential_edited(&Host {
            id: 0,
            ..local.hosts[0].clone()
        })
        .unwrap();
    for (previous, current) in before.hosts.iter().zip(&local.hosts) {
        assert_eq!(current.sync.id, previous.sync.id);
        assert!(current.sync.revision > previous.sync.revision);
    }
}

#[test]
fn rejected_results_cannot_restore_credentials_for_a_host_deleted_during_upload() {
    let folder = Folder::new();
    let mut local = password_host();
    let source = MemoryStore::default();
    source
        .insert(&local.hosts[0], "cloud fixture password")
        .unwrap();
    let mut cloud = Archive::capture(&local).unwrap();
    cloud.capture_credentials(&local, &source).unwrap();
    let destination = MemoryStore::default();
    let (mut restored, _) =
        partial_tests::prepare_upload(local.clone(), Some(cloud), &destination, &folder.0);
    assert!(destination.load(&local.hosts[0]).unwrap().is_none());
    let deleted = local.remove(local.hosts[0].id).unwrap();
    assert!(restored.commit_secrets(&local, &destination).is_err());
    assert!(destination.load(&deleted).unwrap().is_none());
}

#[test]
fn a_credential_changed_after_upload_is_not_overwritten_during_local_application() {
    let folder = Folder::new();
    let local = password_host();
    let mut remote = local.clone();
    remote.upsert(remote.hosts[0].clone()).unwrap();
    let source = MemoryStore::default();
    source
        .insert(&remote.hosts[0], "cloud fixture password")
        .unwrap();
    let mut cloud = Archive::capture(&remote).unwrap();
    cloud.capture_credentials(&remote, &source).unwrap();
    let destination = MemoryStore::default();
    destination
        .insert(&local.hosts[0], "old fixture password")
        .unwrap();
    let (mut restored, _) =
        partial_tests::prepare_upload(local.clone(), Some(cloud), &destination, &folder.0);
    assert_eq!(
        destination.load(&local.hosts[0]).unwrap().unwrap().as_str(),
        "old fixture password"
    );
    destination
        .replace(
            &local.hosts[0],
            "old fixture password",
            "new local fixture password",
        )
        .unwrap();
    assert!(restored.commit_secrets(&local, &destination).is_err());
    assert_eq!(
        destination.load(&local.hosts[0]).unwrap().unwrap().as_str(),
        "new local fixture password"
    );
}
