use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
    time::Duration,
};

use age::secrecy::SecretString;

use super::webdav::Client;

fn server(responses: Vec<&'static str>) -> (Client, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!(
        "http://{}/dav/elyterminal-ssh.age",
        listener.local_addr().unwrap()
    );
    let client = Client::at(
        &url,
        "test",
        &SecretString::from("fixture-password".to_string()),
    )
    .unwrap();
    let task = thread::spawn(move || {
        let mut requests = Vec::new();
        for response in responses {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            loop {
                let mut byte = [0];
                stream.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
                if request.ends_with(b"\r\n\r\n") {
                    break;
                }
            }
            let headers = String::from_utf8(request.clone()).unwrap();
            let length: usize = headers
                .lines()
                .find_map(|line| {
                    line.split_once(':')
                        .filter(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                        .map(|(_, value)| value.trim().parse().unwrap())
                })
                .unwrap_or(0);
            let offset = request.len();
            request.resize(offset + length, 0);
            stream.read_exact(&mut request[offset..]).unwrap();
            requests.push(String::from_utf8(request).unwrap());
            stream.write_all(response.as_bytes()).unwrap();
        }
        requests
    });
    (client, task)
}

#[test]
fn first_upload_uses_create_only_and_existing_uploads_require_the_fetched_etag() {
    let (client, task) = server(vec![
        "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        "HTTP/1.1 201 Created\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        "HTTP/1.1 200 OK\r\nETag: \"revision-1\"\r\nContent-Length: 9\r\nConnection: close\r\n\r\nencrypted",
        "HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n",
    ]);
    assert!(client.get().unwrap().is_none());
    client.put(b"encrypted", None).unwrap();
    let found = client.get().unwrap().unwrap();
    assert_eq!(found.body, b"encrypted");
    client.put(b"encrypted", Some(&found.etag)).unwrap();
    let requests = task.join().unwrap();
    assert!(requests[0].starts_with("GET /dav/elyterminal-ssh.age HTTP/1.1"));
    assert!(requests[0].to_lowercase().contains("authorization: basic "));
    assert!(requests[1].to_lowercase().contains("if-none-match: *"));
    assert!(
        requests[3]
            .to_lowercase()
            .contains("if-match: \"revision-1\"")
    );
}

#[test]
fn concurrent_uploads_and_servers_without_strong_etags_never_overwrite_blindly() {
    let (client, task) = server(vec![
        "HTTP/1.1 412 Precondition Failed\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
    ]);
    assert!(
        client
            .put(b"encrypted", Some("\"old\""))
            .unwrap_err()
            .to_string()
            .contains("another device")
    );
    for etag in ["", "W/\"weak\"", "invalid", "\"a\", \"b\"", "\"a\r\nb\""] {
        assert!(client.put(b"encrypted", Some(etag)).is_err());
    }
    assert_eq!(task.join().unwrap().len(), 1);
}

#[test]
fn authentication_errors_and_redirects_are_reported_without_server_bodies_or_credentials() {
    for response in [
        "HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        "HTTP/1.1 302 Found\r\nLocation: https://elsewhere.example/steal\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
    ] {
        let (client, task) = server(vec![response]);
        let error = client.get().err().unwrap().to_string();
        assert!(!error.contains("fixture-password"));
        assert!(!error.contains("elsewhere"));
        assert_eq!(task.join().unwrap().len(), 1);
    }
}

#[test]
fn download_without_an_etag_can_restore_but_cannot_replace_the_remote_archive() {
    let (client, task) = server(vec![
        "HTTP/1.1 200 OK\r\nContent-Length: 9\r\nConnection: close\r\n\r\nencrypted",
    ]);
    let found = client.get().unwrap().unwrap();
    assert_eq!(found.body, b"encrypted");
    assert!(client.put(b"encrypted", Some(&found.etag)).is_err());
    assert_eq!(task.join().unwrap().len(), 1);
}
