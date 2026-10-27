use super::*;

#[test]
fn credential_identity_is_target_and_method_specific_not_a_display_name_or_id() {
    let host = Host::default();
    assert_eq!(account(&host).len(), 64);
    assert_eq!(
        account(&host),
        account(&Host {
            id: 19,
            name: "Renamed".into(),
            ..host.clone()
        })
    );
    for other in [
        Host {
            address: "localhost".into(),
            ..host.clone()
        },
        Host {
            port: 2222,
            ..host.clone()
        },
        Host {
            user: "root".into(),
            ..host.clone()
        },
        Host {
            auth: Authentication::Password,
            ..host.clone()
        },
    ] {
        assert_ne!(account(&host), account(&other));
    }
}

#[test]
fn explicit_modes_do_not_fall_through_to_other_authentication_methods() {
    assert!(
        Authentication::Password
            .options()
            .iter()
            .any(|arg| arg == "PubkeyAuthentication=no")
    );
    assert!(
        Authentication::PrivateKey
            .options()
            .iter()
            .any(|arg| arg == "IdentitiesOnly=yes")
    );
    assert!(
        Authentication::Agent
            .options()
            .iter()
            .any(|arg| arg == "IdentityFile=none")
    );
    assert!(Authentication::OpenSsh.options().is_empty());
}
