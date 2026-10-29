use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
    thread,
    time::Duration,
};

use age::secrecy::SecretString;

use super::*;

struct Folder(PathBuf);

impl Drop for Folder {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn uploading_on_one_machine_and_restoring_on_another_transfers_the_actual_key() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!(
        "http://{}/dav/elyterminal-ssh.age",
        listener.local_addr().unwrap()
    );
    let server = thread::spawn(move || {
        let mut archive = Vec::new();
        for step in 0..3 {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut headers = Vec::new();
            while !headers.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                stream.read_exact(&mut byte).unwrap();
                headers.push(byte[0]);
                assert!(headers.len() < 65536);
            }
            let headers = String::from_utf8(headers).unwrap();
            match step {
                0 => {
                    assert!(headers.starts_with("GET "));
                    stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
                }
                1 => {
                    assert!(headers.starts_with("PUT "));
                    assert!(headers.to_lowercase().contains("if-none-match: *"));
                    let length: usize = headers
                        .lines()
                        .find_map(|line| {
                            line.split_once(':')
                                .filter(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                                .map(|(_, value)| value.trim().parse().unwrap())
                        })
                        .unwrap();
                    archive.resize(length, 0);
                    stream.read_exact(&mut archive).unwrap();
                    stream.write_all(b"HTTP/1.1 201 Created\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
                }
                2 => {
                    assert!(headers.starts_with("GET "));
                    write!(stream, "HTTP/1.1 200 OK\r\nETag: \"one\"\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", archive.len()).unwrap();
                    stream.write_all(&archive).unwrap();
                }
                _ => unreachable!(),
            }
        }
        archive
    });
    let folder =
        Folder(std::env::temp_dir().join(format!("ely-webdav-flow-{}", std::process::id())));
    fs::create_dir_all(&folder.0).unwrap();
    let original = folder.0.join("machine-a.key");
    let private = b"-----BEGIN OPENSSH PRIVATE KEY-----\nTHIS IS ONLY A TEST FIXTURE\n-----END OPENSSH PRIVATE KEY-----\n";
    fs::write(&original, private).unwrap();
    let mut local = Connections::default();
    local
        .upsert(Host {
            address: "transfer.example".into(),
            identity: original.to_str().unwrap().into(),
            ..Host::default()
        })
        .unwrap();
    let profile = saved::Profile {
        webdav: WebDav {
            url: "https://example.com/dav".into(),
            username: "fixture".into(),
        },
        settings: SyncSettings {
            remember_password: true,
            remember_passphrase: true,
            auto_sync: true,
            ..SyncSettings::default()
        },
    };
    let store = saved::VaultStore(crate::ssh::vault::Vault::at(
        folder.0.join("sync-credentials"),
    ));
    profile
        .remember(
            &profile,
            &saved::Secrets {
                password: SecretString::from("test only".to_string()),
                passphrase: SecretString::from("635841".to_string()),
            },
            &store,
        )
        .unwrap();
    let request = |local, key_folder, restore_only| {
        let secrets = profile
            .resolve(saved::Secrets::default(), true, &store)
            .unwrap();
        Request {
            gitee: None,
            include_credentials: false,
            remember_token: false,
            settings: profile.webdav.clone(),
            password: secrets.password,
            passphrase: secrets.passphrase,
            local,
            key_folder,
            restore_only,
        }
    };
    let client = || {
        webdav::Client::at(
            &url,
            "fixture",
            &SecretString::from("test only".to_string()),
        )
        .unwrap()
    };
    let saved = synchronize(request(local, folder.0.join("machine-a"), false), client()).unwrap();
    assert_eq!(saved.keys, 1);
    fs::remove_file(original).unwrap();
    let mut target = Connections::default();
    let restored = synchronize(
        request(target.clone(), folder.0.join("machine-b"), true),
        client(),
    )
    .unwrap();
    restored.apply(&mut target).unwrap();
    assert_eq!(target.hosts.len(), 1);
    assert_eq!(target.hosts[0].address, "transfer.example");
    assert_eq!(fs::read(&target.hosts[0].identity).unwrap(), private);
    assert!(PathBuf::from(&target.hosts[0].identity).starts_with(folder.0.join("machine-b")));
    let ciphertext = server.join().unwrap();
    assert!(ciphertext.starts_with(b"age-encryption.org/v1\n"));
    for secret in [
        b"transfer.example".as_slice(),
        b"THIS IS ONLY A TEST FIXTURE".as_slice(),
    ] {
        assert!(!ciphertext.windows(secret.len()).any(|part| part == secret));
    }
}

#[test]
fn restore_recovers_a_missing_local_password_without_uploading_an_incomplete_archive() {
    use super::credential_tests::{MemoryStore, password_host};
    use super::credentials::Store;

    let local = password_host();
    let source = MemoryStore::default();
    source
        .insert(&local.hosts[0], "fixture-ssh-password")
        .unwrap();
    let mut archive = Archive::capture(&local).unwrap();
    archive.capture_credentials(&local, &source).unwrap();
    let passphrase = SecretString::from("635841".to_owned());
    let encrypted = crypto::encrypt(&archive, &passphrase).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/archive", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut headers = Vec::new();
        while !headers.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            stream.read_exact(&mut byte).unwrap();
            headers.push(byte[0]);
            assert!(headers.len() < 65536);
        }
        assert!(headers.starts_with(b"GET "));
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nETag: \"one\"\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            encrypted.len()
        )
        .unwrap();
        stream.write_all(&encrypted).unwrap();
        listener.set_nonblocking(true).unwrap();
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    });
    let destination = MemoryStore::default();
    let request = Request {
        settings: WebDav::default(),
        gitee: None,
        include_credentials: true,
        remember_token: false,
        password: SecretString::from("fixture-token".to_owned()),
        passphrase,
        local,
        key_folder: std::env::temp_dir(),
        restore_only: true,
    };
    let client = webdav::Client::at(&url, "fixture", &request.password).unwrap();
    let restored = synchronize_with(request, Remote::WebDav(client), &destination).unwrap();
    assert_eq!(restored.credentials, 1);
    assert!(restored.connections.hosts[0].remember_secret);
    assert_eq!(
        destination
            .load(&restored.connections.hosts[0])
            .unwrap()
            .unwrap()
            .as_str(),
        "fixture-ssh-password"
    );
    server.join().unwrap();
}
