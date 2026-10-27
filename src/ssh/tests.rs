use super::*;

fn host() -> Host {
    Host {
        name: "Debian LAN".into(),
        address: "192.168.0.57".into(),
        user: "root".into(),
        group: "Homelab".into(),
        ..Host::default()
    }
}

#[test]
fn a_new_host_defaults_to_loopback_without_replacing_saved_addresses() {
    assert_eq!(Host::default().address, "127.0.0.1");
    let saved = host();
    let loaded: Host = serde_json::from_str(&serde_json::to_string(&saved).unwrap()).unwrap();
    assert_eq!(loaded.address, "192.168.0.57");
}

#[test]
fn ssh_runs_directly_with_arguments_not_through_a_shell() {
    assert_eq!(
        host().arguments().unwrap(),
        ["-tt", "-p", "22", "-l", "root", "--", "192.168.0.57"]
    );
}

#[test]
fn host_options_and_shell_syntax_are_not_destinations() {
    for address in [
        "",
        "-oProxyCommand=anything",
        "host;echo",
        "user@host",
        "a b",
        "a\nb",
        "$(id)",
        "ssh://host",
    ] {
        let host = Host {
            address: address.into(),
            ..host()
        };
        assert!(host.arguments().is_err(), "{address:?}");
    }
}

#[test]
fn ipv6_aliases_and_custom_ports_keep_their_meaning() {
    let host = Host {
        address: "fe80::1%en0".into(),
        port: 2222,
        ..host()
    };
    assert_eq!(host.destination(), "root@[fe80::1%en0]:2222");
    assert_eq!(host.arguments().unwrap().last().unwrap(), "fe80::1%en0");
    assert!(
        Host {
            address: "my-build-host".into(),
            ..host
        }
        .validate()
        .is_ok()
    );
}

#[test]
fn an_empty_user_leaves_the_login_to_openssh() {
    let args = Host {
        user: String::new(),
        ..host()
    }
    .arguments()
    .unwrap();
    assert_eq!(args, ["-tt", "-p", "22", "--", "192.168.0.57"]);
}

#[test]
fn identity_paths_with_spaces_stay_one_argument() {
    let identity = std::env::temp_dir()
        .join("SSH keys")
        .join("id_ed25519")
        .display()
        .to_string();
    let args = Host {
        identity: identity.clone(),
        ..host()
    }
    .arguments()
    .unwrap();
    assert_eq!(args[5..], ["-i", &identity, "--", "192.168.0.57"]);
}

#[test]
fn invalid_ports_names_users_and_identity_paths_are_errors() {
    for host in [
        Host { port: 0, ..host() },
        Host {
            name: "a\nb".into(),
            ..host()
        },
        Host {
            group: "a\tb".into(),
            ..host()
        },
        Host {
            user: "root\nanything".into(),
            ..host()
        },
        Host {
            identity: "relative/key".into(),
            ..host()
        },
    ] {
        assert!(host.validate().is_err());
    }
}

#[test]
fn profiles_are_edited_by_id_and_deleted_ids_are_not_reused() {
    let mut data = Connections::default();
    let first = data.upsert(host()).unwrap();
    let mut edited = first.clone();
    edited.name = "Renamed".into();
    data.upsert(edited).unwrap();
    assert_eq!(data.hosts.len(), 1);
    assert_eq!(data.hosts[0].name, "Renamed");
    data.hosts.clear();
    let second = data.upsert(host()).unwrap();
    assert_ne!(first.id, second.id);
    assert!(data.upsert(first).is_err());
}

#[test]
fn recent_connections_are_deduplicated_by_target_not_name() {
    let mut data = Connections::default();
    data.opened(host(), 1);
    data.opened(
        Host {
            name: "Another name".into(),
            ..host()
        },
        2,
    );
    assert_eq!(data.recent.len(), 1);
    assert_eq!(data.recent[0].opened_at, 2);
    data.opened(
        Host {
            user: "debian".into(),
            ..host()
        },
        3,
    );
    data.opened(
        Host {
            port: 2222,
            ..host()
        },
        4,
    );
    assert_eq!(data.recent.len(), 3);
    assert_eq!(data.recent[0].host.port, 2222);
}

#[test]
fn recent_connections_are_bounded() {
    let mut data = Connections::default();
    for port in 1..=30 {
        data.opened(Host { port, ..host() }, u64::from(port));
    }
    assert_eq!(data.recent.len(), 20);
    assert_eq!(data.recent[0].host.port, 30);
    assert_eq!(data.recent[19].host.port, 11);
}

#[test]
fn searches_cover_names_addresses_users_and_groups() {
    for query in ["debian", "168.0", "root", "homelab", ""] {
        assert!(host().matches(query));
    }
    assert!(!host().matches("missing"));
}

#[test]
fn duplicate_or_unallocated_ids_are_rejected() {
    let mut data = Connections::default();
    let first = data.upsert(host()).unwrap();
    data.hosts.push(first);
    assert!(data.validate().is_err());
}

#[test]
fn connections_save_atomically_and_load_without_credentials() {
    let folder = std::env::temp_dir().join(format!("elyterminal-ssh-store-{}", std::process::id()));
    let path = folder.join("connections.json");
    let mut data = Connections::default();
    data.upsert(host()).unwrap();
    data.opened(host(), 123);
    store::save(&path, &data).unwrap();
    assert_eq!(store::load(&path).unwrap(), data);
    data.upsert(Host {
        name: "Second".into(),
        ..host()
    })
    .unwrap();
    store::save(&path, &data).unwrap();
    assert_eq!(store::load(&path).unwrap(), data);
    assert_eq!(std::fs::read_dir(&folder).unwrap().count(), 1);
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(!text.contains("password"));
    assert!(!text.contains("PRIVATE KEY"));
    std::fs::remove_dir_all(folder).unwrap();
}

#[test]
fn malformed_or_credential_fields_are_not_accepted() {
    assert!(serde_json::from_str::<Connections>(r#"{"password":"secret"}"#).is_err());
    let mut json = serde_json::to_value(host()).unwrap();
    json["password"] = serde_json::Value::String("not allowed".into());
    assert!(serde_json::from_value::<Host>(json).is_err());
}

#[test]
fn a_bad_connection_file_is_never_replaced_by_defaults() {
    let folder = std::env::temp_dir().join(format!("elyterminal-ssh-bad-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let path = folder.join("connections.json");
    std::fs::write(&path, "not JSON").unwrap();
    assert!(store::load(&path).is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "not JSON");
    std::fs::remove_dir_all(folder).unwrap();
}

#[test]
fn a_missing_identity_stops_before_starting_ssh() {
    let missing = std::env::temp_dir().join("elyterminal-key-does-not-exist");
    let host = Host {
        identity: missing.display().to_string(),
        ..host()
    };
    let error = host.launch(std::env::temp_dir()).unwrap_err();
    assert!(error.to_string().contains("identity file does not exist"));
}
