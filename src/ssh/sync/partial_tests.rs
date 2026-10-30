use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    thread,
    time::{Duration, Instant},
};

use age::secrecy::SecretString;

use super::{
    credential_tests::{MemoryStore, password_host},
    credentials::Store,
    *,
};
use crate::ssh::Authentication;

pub(super) fn upload(
    local: Connections,
    cloud: Option<Archive>,
    store: &impl Store,
    folder: &std::path::Path,
) -> (Restored, Archive) {
    let passphrase = SecretString::from("635841".to_owned());
    let cloud = cloud.map(|archive| crypto::encrypt(&archive, &passphrase).unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let client = webdav::Client::at(
        &format!("http://{}/archive", listener.local_addr().unwrap()),
        "fixture",
        &SecretString::from("fixture token".to_owned()),
    )
    .unwrap();
    let server = thread::spawn(move || {
        for step in 0..2 {
            let until = Instant::now() + Duration::from_secs(10);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(
                            Instant::now() < until,
                            "sync never contacted fixture server"
                        );
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => panic!("fixture accept: {error}"),
                }
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            let mut headers = Vec::new();
            while !headers.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                stream.read_exact(&mut byte).unwrap();
                headers.push(byte[0]);
                assert!(headers.len() < 65536);
            }
            let headers = String::from_utf8(headers).unwrap();
            if step == 0 {
                assert!(headers.starts_with("GET "));
                if let Some(body) = &cloud {
                    write!(stream, "HTTP/1.1 200 OK\r\nETag: \"fixture\"\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).unwrap();
                    stream.write_all(body).unwrap();
                } else {
                    stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
                }
            } else {
                assert!(headers.starts_with("PUT "));
                let lower = headers.to_ascii_lowercase();
                assert!(lower.contains(if cloud.is_some() {
                    "if-match: \"fixture\""
                } else {
                    "if-none-match: *"
                }));
                let length: usize = lower
                    .lines()
                    .find_map(|line| {
                        line.strip_prefix("content-length:")
                            .map(|value| value.trim().parse().unwrap())
                    })
                    .unwrap();
                assert!(length <= LIMIT);
                let mut body = vec![0; length];
                stream.read_exact(&mut body).unwrap();
                stream.write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
                return body;
            }
        }
        unreachable!("fixture receives GET and PUT")
    });
    let result = synchronize_with(
        Request {
            settings: WebDav::default(),
            gitee: None,
            include_credentials: true,
            remember_token: false,
            password: SecretString::from("fixture token".to_owned()),
            passphrase: passphrase.clone(),
            local,
            key_folder: folder.join("restored"),
            restore_only: false,
        },
        Remote::WebDav(client),
        store,
    );
    let uploaded = server.join().unwrap();
    let archive = crypto::decrypt(&uploaded, &passphrase).unwrap();
    (result.unwrap(), archive)
}

#[test]
fn incomplete_hosts_do_not_block_a_first_upload_or_change_local_profiles() {
    let folder = tests::Folder::new();
    let mut local = password_host();
    let available = local
        .upsert(Host {
            id: 0,
            address: "available.fixture.invalid".into(),
            ..local.hosts[0].clone()
        })
        .unwrap();
    for auth in [Authentication::Agent, Authentication::OpenSsh] {
        local
            .upsert(Host {
                address: format!("{}.fixture.invalid", auth.id()),
                auth,
                ..Host::default()
            })
            .unwrap();
    }
    let key_host = folder.local("available-key").hosts.remove(0);
    local
        .upsert(Host {
            id: 0,
            address: "keys.fixture.invalid".into(),
            auth: Authentication::PrivateKey,
            remember_secret: true,
            ..key_host
        })
        .unwrap();
    local
        .upsert(Host {
            id: 0,
            identity: folder.0.join("missing-key").to_str().unwrap().into(),
            ..local.hosts[4].clone()
        })
        .unwrap();
    let store = MemoryStore::default();
    store
        .insert(&available, "available fixture password")
        .unwrap();
    let (restored, archive) = upload(local.clone(), None, &store, &folder.0);
    assert_eq!(
        (restored.hosts, restored.keys, restored.credentials),
        (5, 1, 1)
    );
    assert_eq!(restored.warnings.len(), 2);
    assert!(
        restored
            .warnings
            .iter()
            .any(|text| text.contains("selected private key file is missing"))
    );
    assert!(
        restored
            .warnings
            .iter()
            .any(|text| text.contains("credential-fixture.example"))
    );
    assert!(
        !restored
            .warnings
            .join("\n")
            .contains("available fixture password")
    );
    assert_eq!(restored.connections, local);
    let uploaded = archive
        .restore(Connections::default(), &folder.0.join("another-device"))
        .unwrap();
    assert_eq!(uploaded.hosts.len(), 5);
    assert!(
        uploaded
            .hosts
            .iter()
            .any(|host| host.address == local.hosts[0].address)
    );
    assert_eq!(archive.credentials.len(), 1);
}

#[test]
fn missing_local_secrets_are_recovered_from_cloud_instead_of_erasing_them() {
    let folder = tests::Folder::new();
    let mut local = password_host();
    let key_host = folder.local("cloud-key").hosts.remove(0);
    local
        .upsert(Host {
            id: 0,
            auth: Authentication::PrivateKey,
            remember_secret: true,
            ..key_host
        })
        .unwrap();
    let source = MemoryStore::default();
    source
        .insert(&local.hosts[0], "cloud fixture password")
        .unwrap();
    source
        .insert(&local.hosts[1], "cloud fixture key phrase")
        .unwrap();
    let mut cloud = Archive::capture(&local).unwrap();
    cloud.capture_credentials(&local, &source).unwrap();
    fs::remove_file(&local.hosts[1].identity).unwrap();
    let destination = MemoryStore::default();
    let (restored, uploaded) = upload(local, Some(cloud), &destination, &folder.0);
    assert_eq!(
        (restored.hosts, restored.keys, restored.credentials),
        (2, 1, 2)
    );
    assert_eq!(restored.warnings.len(), 2);
    assert_eq!(
        (
            uploaded.hosts.len(),
            uploaded.keys.len(),
            uploaded.credentials.len()
        ),
        (2, 1, 2)
    );
    for host in &restored.connections.hosts {
        let expected = if host.auth == Authentication::Password {
            "cloud fixture password"
        } else {
            assert!(std::path::Path::new(&host.identity).is_file());
            "cloud fixture key phrase"
        };
        assert!(host.remember_secret);
        assert_eq!(destination.load(host).unwrap().unwrap().as_str(), expected);
    }
}

#[test]
fn incomplete_password_hosts_sync_without_credentials_when_export_is_disabled() {
    let mut local = password_host();
    let (archive, warnings) =
        Archive::capture_available(&local, &MemoryStore::default(), false).unwrap();
    assert_eq!(archive.hosts.len(), 1);
    assert!(archive.credentials.is_empty());
    assert!(warnings.is_empty());
    local.hosts[0].remember_secret = false;
    let (archive, warnings) =
        Archive::capture_available(&local, &MemoryStore::default(), true).unwrap();
    assert_eq!(archive.hosts.len(), 1);
    assert!(archive.credentials.is_empty());
    assert!(warnings.is_empty());
}

#[test]
fn invalid_existing_key_files_are_not_treated_as_missing() {
    let folder = tests::Folder::new();
    let local = folder.local("invalid-key");
    fs::write(&local.hosts[0].identity, "not a private key").unwrap();
    assert!(Archive::capture_available(&local, &MemoryStore::default(), true).is_err());
}

#[cfg(unix)]
#[test]
fn unreadable_key_files_are_not_treated_as_missing() {
    use std::os::unix::fs::PermissionsExt as _;
    let folder = tests::Folder::new();
    let local = folder.local("unreadable-key");
    let path = &local.hosts[0].identity;
    fs::set_permissions(path, fs::Permissions::from_mode(0o000)).unwrap();
    let result = Archive::capture_available(&local, &MemoryStore::default(), true);
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(result.is_err());
}
