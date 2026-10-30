use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    thread,
    time::{Duration, Instant},
};

use serde_json::{Value, json};

use super::*;
use crate::ssh::{
    Connections, Host,
    sync::{
        Remote, Request,
        credential_tests::{MemoryStore, password_host},
        credentials::Store,
        synchronize_with,
    },
};

struct Captured {
    headers: String,
    body: Vec<u8>,
}

fn server(
    count: usize,
    mut respond: impl FnMut(usize, &Captured) -> (u16, Value) + Send + 'static,
) -> (Client, thread::JoinHandle<Vec<Captured>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let client = Client::at(
        &format!("http://{}", listener.local_addr().unwrap()),
        &Gitee::default(),
        &SecretString::from("fixture-token".to_string()),
    )
    .unwrap();
    let task = thread::spawn(move || {
        let mut requests = Vec::new();
        for step in 0..count {
            let deadline = Instant::now() + Duration::from_secs(15);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline, "fixture request timed out");
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => panic!("{error}"),
                }
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            let mut reader = BufReader::new(&mut stream);
            let mut headers = String::new();
            loop {
                let mut line = String::new();
                assert!(reader.read_line(&mut line).unwrap() > 0);
                headers.push_str(&line);
                assert!(headers.len() < 65536);
                if line == "\r\n" {
                    break;
                }
            }
            let length = headers
                .lines()
                .find_map(|line| {
                    line.split_once(':')
                        .filter(|(key, _)| key.eq_ignore_ascii_case("content-length"))
                        .map(|(_, value)| value.trim().parse::<usize>().unwrap())
                })
                .unwrap_or(0);
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            let request = Captured { headers, body };
            let (status, body) = respond(step, &request);
            let body = serde_json::to_vec(&body).unwrap();
            write!(stream, "HTTP/1.1 {status} Fixture\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).unwrap();
            stream.write_all(&body).unwrap();
            requests.push(request);
        }
        requests
    });
    (client, task)
}

fn snippet(id: &str, encrypted: &[u8]) -> Value {
    json!({"id": id, "description": "MioTerm Sync main", "public": false, "created_at": "2026-01-01T00:00:00Z",
        "files": {FILE: {"content": STANDARD.encode(encrypted), "truncated": false}}})
}

fn phrase() -> SecretString {
    SecretString::from("635841".to_string())
}

#[test]
fn gitee_transfers_encrypted_hosts_and_passwords_without_storing_tokens_in_urls() {
    let mut snapshot = Value::Null;
    let (client, task) = server(6, move |step, request| match step {
        0 => (200, json!([])),
        1 => {
            assert!(request.headers.starts_with("POST /gists "));
            snapshot = serde_json::from_slice(&request.body).unwrap();
            assert_eq!(snapshot["public"], false);
            assert!(!String::from_utf8_lossy(&request.body).contains("fixture password only"));
            snapshot["id"] = json!("saved");
            (201, snapshot.clone())
        }
        2 | 4 => (200, json!([snapshot.clone()])),
        3 | 5 => (200, snapshot.clone()),
        _ => unreachable!(),
    });
    let base = client.base.clone();
    let folder = crate::ssh::sync::tests::Folder::new();
    let mut local = folder.local("gitee-key-fixture");
    local.hosts[0].auth = crate::ssh::Authentication::PrivateKey;
    local.hosts[0].remember_secret = true;
    let mut password = password_host().hosts.remove(0);
    password.id = 0;
    local.upsert(password).unwrap();
    local
        .upsert(Host {
            id: 0,
            address: "missing-password.fixture.invalid".into(),
            ..local.hosts[1].clone()
        })
        .unwrap();
    local
        .upsert(Host {
            id: 0,
            identity: folder.0.join("missing-key").to_str().unwrap().into(),
            address: "missing-key.fixture.invalid".into(),
            ..local.hosts[0].clone()
        })
        .unwrap();
    let original = local.hosts[0].identity.clone();
    let source = MemoryStore::default();
    source
        .insert(&local.hosts[0], "fixture password only")
        .unwrap();
    source
        .insert(&local.hosts[1], "fixture SSH password")
        .unwrap();
    let request = |local, restore_only| Request {
        settings: super::super::WebDav::default(),
        gitee: Some(Gitee::default()),
        include_credentials: true,
        remember_token: false,
        password: SecretString::from("fixture-token".to_string()),
        passphrase: phrase(),
        local,
        restore_only,
        key_folder: folder.0.join("restored-keys"),
    };
    let uploaded = synchronize_with(request(local, false), Remote::Gitee(client), &source).unwrap();
    assert_eq!(
        (uploaded.hosts, uploaded.keys, uploaded.credentials),
        (3, 1, 2)
    );
    assert_eq!(uploaded.warnings.len(), 2);
    assert_eq!(uploaded.connections.hosts.len(), 4);
    std::fs::remove_file(original).unwrap();
    let client = Client::at(
        &base,
        &Gitee::default(),
        &SecretString::from("a-rotated-fixture-token".to_string()),
    )
    .unwrap();
    let target = MemoryStore::default();
    let restored = synchronize_with(
        request(Connections::default(), true),
        Remote::Gitee(client),
        &target,
    )
    .unwrap();
    assert_eq!(restored.hosts, 3);
    assert_eq!(restored.credentials, 2);
    assert_eq!(restored.keys, 1);
    let missing_password = restored
        .connections
        .hosts
        .iter()
        .find(|host| host.address == "missing-password.fixture.invalid")
        .unwrap();
    assert!(!missing_password.remember_secret);
    assert!(target.load(missing_password).unwrap().is_none());
    let key_host = restored
        .connections
        .hosts
        .iter()
        .find(|host| host.auth == crate::ssh::Authentication::PrivateKey)
        .unwrap();
    assert!(std::path::Path::new(&key_host.identity).starts_with(folder.0.join("restored-keys")));
    assert!(
        std::fs::read_to_string(&key_host.identity)
            .unwrap()
            .contains("TEST FIXTURE")
    );
    assert_eq!(
        target.load(key_host).unwrap().unwrap().as_str(),
        "fixture password only"
    );
    let mut deleted = restored.connections.clone();
    deleted.remove(key_host.id).unwrap();
    let client = Client::at(
        &base,
        &Gitee::default(),
        &SecretString::from("fixture-token".to_string()),
    )
    .unwrap();
    let destination = MemoryStore::default();
    let filtered =
        synchronize_with(request(deleted, true), Remote::Gitee(client), &destination).unwrap();
    assert_eq!(
        (filtered.hosts, filtered.keys, filtered.credentials),
        (2, 0, 1)
    );
    assert_eq!(filtered.connections.hosts.len(), 2);
    assert!(destination.load(key_host).unwrap().is_none());
    let requests = task.join().unwrap();
    for request in requests {
        let first = request.headers.lines().next().unwrap();
        assert!(!first.contains("token"));
        assert!(
            request
                .headers
                .to_lowercase()
                .contains("authorization: bearer ")
        );
        assert!(!first.starts_with("PATCH"));
    }
}

#[test]
fn concurrent_snapshots_are_merged_and_authenticated_history_skips_covered_snapshots() {
    let mut left = Connections::default();
    left.upsert(Host {
        address: "left.example".into(),
        ..Host::default()
    })
    .unwrap();
    let mut right = Connections::default();
    right
        .upsert(Host {
            address: "right.example".into(),
            ..Host::default()
        })
        .unwrap();
    let one = snippet(
        "one",
        &crypto::encrypt(&Archive::capture(&left).unwrap(), &phrase()).unwrap(),
    );
    let two = snippet(
        "two",
        &crypto::encrypt(&Archive::capture(&right).unwrap(), &phrase()).unwrap(),
    );
    let (client, task) = server(3, move |step, request| match step {
        0 => (200, json!([one.clone(), two.clone()])),
        _ if request.headers.starts_with("GET /gists/one ") => (200, one.clone()),
        _ => (200, two.clone()),
    });
    let merged = client.pull(&phrase()).unwrap().unwrap();
    assert_eq!(merged.hosts.len(), 2);
    assert_eq!(merged.snapshots.len(), 2);
    task.join().unwrap();
    let mut newest = snippet("zzz", &crypto::encrypt(&merged, &phrase()).unwrap());
    newest["created_at"] = json!("2026-02-01T00:00:00Z");
    let (client, task) = server(2, move |step, _| {
        if step == 0 {
            let mut old = newest.clone();
            old["id"] = json!("one");
            let mut other = newest.clone();
            other["id"] = json!("two");
            (200, json!([old, newest.clone(), other]))
        } else {
            (200, newest.clone())
        }
    });
    assert_eq!(client.pull(&phrase()).unwrap().unwrap().hosts.len(), 2);
    assert_eq!(task.join().unwrap().len(), 2);
}

#[test]
fn public_truncated_missing_and_wrongly_encrypted_snapshots_are_refused() {
    let valid = snippet(
        "one",
        &crypto::encrypt(&Archive::default(), &phrase()).unwrap(),
    );
    let mut cases = Vec::new();
    let mut public = valid.clone();
    public["public"] = json!(true);
    cases.push(public);
    let mut truncated = valid.clone();
    truncated["files"][FILE]["truncated"] = json!(true);
    cases.push(truncated);
    let mut missing = valid.clone();
    missing["files"][FILE] = json!({"raw_url":"https://untrusted.example"});
    cases.push(missing);
    let mut invalid = valid.clone();
    invalid["files"][FILE]["content"] = json!("not base64");
    cases.push(invalid);
    let wrong = crypto::encrypt(
        &Archive::default(),
        &SecretString::from("different fixture phrase".to_string()),
    )
    .unwrap();
    cases.push(snippet("one", &wrong));
    for broken in cases {
        let valid = valid.clone();
        let (client, task) = server(2, move |step, _| {
            if step == 0 {
                (200, json!([valid.clone()]))
            } else {
                (200, broken.clone())
            }
        });
        assert!(client.pull(&phrase()).is_err());
        task.join().unwrap();
    }
}

#[test]
fn pagination_and_http_errors_never_result_in_partial_or_unprotected_uploads() {
    let (client, task) = server(2, |step, _| {
        if step == 0 {
            (
                200,
                json!(
                    (0..100)
                        .map(|id| json!({"id":format!("{id}"),"public":true,"description":"other"}))
                        .collect::<Vec<_>>()
                ),
            )
        } else {
            (200, json!([]))
        }
    });
    assert!(client.pull(&phrase()).unwrap().is_none());
    assert!(
        task.join().unwrap()[1]
            .headers
            .starts_with("GET /gists?page=2")
    );
    for status in [302, 401, 403, 429, 500] {
        let (client, task) = server(1, move |_, _| {
            (status, json!({"message":"secret response fixture"}))
        });
        let error = client.pull(&phrase()).err().unwrap().to_string();
        assert!(!error.contains("fixture-token") && !error.contains("secret response fixture"));
        task.join().unwrap();
    }
    let (client, task) = server(1, |_, _| {
        (
            201,
            json!({"id":"public","public":true,"description":"MioTerm Sync main"}),
        )
    });
    assert!(client.put(b"encrypted fixture").is_err());
    assert!(client.put(&vec![0; SNAPSHOT_LIMIT + 1]).is_err());
    assert_eq!(task.join().unwrap().len(), 1);
}
