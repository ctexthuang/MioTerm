use std::{fs, path::Path, process::Command};

use age::secrecy::SecretString;
use base64::{Engine as _, engine::general_purpose::STANDARD};

use super::{
    archive::Archive, credential_tests::MemoryStore, credentials::Store, crypto, tests::Folder,
};
use crate::ssh::{Authentication, Connections, Host, keys::Key};

#[test]
fn imported_rsa_pem_and_certificate_round_trip_as_encrypted_json_without_the_source() {
    let folder = Folder::new();
    let source = folder.0.join("external.pem");
    let passphrase = "temporary PEM fixture passphrase";
    generate(&source, "rsa", passphrase);
    let expected = fs::read(&source).unwrap();
    assert!(expected.starts_with(b"-----BEGIN RSA PRIVATE KEY-----"));
    let authority = folder.0.join("authority");
    generate(&authority, "ed25519", "");
    let public = source.with_extension("pem.pub");
    assert!(
        Command::new("ssh-keygen")
            .args(["-q", "-s"])
            .arg(&authority)
            .args(["-I", "pem-import-fixture", "-V", "+1h"])
            .arg(&public)
            .output()
            .unwrap()
            .status
            .success()
    );
    let certificate = folder.0.join("external.pem-cert.pub");
    let expected_certificate = fs::read(&certificate).unwrap();
    let key = Key::capture(&source).unwrap();
    let imported = key.install(&folder.0.join("source-app/keys")).unwrap();
    fs::remove_file(&source).unwrap();
    fs::remove_file(certificate).unwrap();

    let mut local = Connections::default();
    let host = local
        .upsert(Host {
            address: "pem-sync.fixture.invalid".into(),
            identity: imported.clone(),
            auth: Authentication::PrivateKey,
            remember_secret: true,
            ..Host::default()
        })
        .unwrap();
    let store = MemoryStore::default();
    store.insert(&host, passphrase).unwrap();
    let mut archive = Archive::capture(&local).unwrap();
    archive.capture_credentials(&local, &store).unwrap();
    let json = serde_json::to_value(&archive).unwrap();
    let material = json["keys"].as_object().unwrap().values().next().unwrap();
    assert_eq!(
        STANDARD
            .decode(material["private"].as_str().unwrap())
            .unwrap(),
        expected
    );
    assert!(
        !serde_json::to_string(&json)
            .unwrap()
            .contains(&folder.0.display().to_string())
    );
    let phrase = SecretString::from("635841".to_owned());
    let encrypted = crypto::encrypt(&archive, &phrase).unwrap();
    assert!(
        !encrypted
            .windows(passphrase.len())
            .any(|part| part == passphrase.as_bytes())
    );
    fs::remove_file(imported).unwrap();
    let received = crypto::decrypt(&encrypted, &phrase).unwrap();
    let mut restored = received
        .restore(Connections::default(), &folder.0.join("other-os/keys"))
        .unwrap();
    let target = MemoryStore::default();
    assert_eq!(
        received
            .restore_credentials(&mut restored, &target)
            .unwrap(),
        1
    );
    let host = &restored.hosts[0];
    let secret = target.load(host).unwrap().unwrap();
    let output = Command::new("ssh-keygen")
        .args(["-y", "-P", &secret, "-f", &host.identity])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "restored encrypted PEM is readable by OpenSSH"
    );
    let fields = |bytes: &[u8]| {
        bytes
            .split(|b| b.is_ascii_whitespace())
            .take(2)
            .map(<[u8]>::to_vec)
            .collect::<Vec<_>>()
    };
    assert_eq!(fields(&output.stdout), fields(&fs::read(public).unwrap()));
    assert_eq!(
        fs::read(format!("{}-cert.pub", host.identity)).unwrap(),
        expected_certificate
    );
}

#[test]
fn concurrent_imports_reuse_the_same_complete_key() {
    let folder = Folder::new();
    let local = folder.local("concurrent-key");
    let source = Path::new(&local.hosts[0].identity);
    let managed = folder.0.join("managed");
    let paths = std::thread::scope(|scope| {
        let jobs = (0..8)
            .map(|_| scope.spawn(|| Key::capture(source).unwrap().install(&managed).unwrap()))
            .collect::<Vec<_>>();
        jobs.into_iter()
            .map(|job| job.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert!(paths.iter().all(|path| path == &paths[0]));
    assert_eq!(fs::read(&paths[0]).unwrap(), fs::read(source).unwrap());
}

#[test]
fn interrupted_key_import_is_retried_without_exposing_a_partial_identity() {
    let folder = Folder::new();
    let local = folder.local("interrupted-key");
    let source = Path::new(&local.hosts[0].identity);
    let key = Key::capture(source).unwrap();
    let managed = folder.0.join("managed");
    crate::ssh::private::folder(&managed).unwrap();
    let path = managed.join(key.id());
    let temp = path.with_extension("tmp");
    fs::write(&temp, "interrupted import").unwrap();
    assert!(!path.exists());
    key.install(&managed).unwrap();
    assert!(!temp.exists());
    assert_eq!(fs::read(&path).unwrap(), fs::read(source).unwrap());
    fs::write(&path, "different contents").unwrap();
    assert!(key.install(&managed).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"different contents");
}

#[cfg(unix)]
#[test]
fn interrupted_import_cannot_replace_a_temporary_symlink_or_hardlink() {
    let folder = Folder::new();
    let local = folder.local("interrupted-key-links");
    let source = Path::new(&local.hosts[0].identity);
    let key = Key::capture(source).unwrap();
    let managed = folder.0.join("managed");
    crate::ssh::private::folder(&managed).unwrap();
    let temp = managed.join(key.id()).with_extension("tmp");
    std::os::unix::fs::symlink(source, &temp).unwrap();
    assert!(key.install(&managed).is_err());
    assert!(
        fs::symlink_metadata(&temp)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    fs::remove_file(&temp).unwrap();
    fs::hard_link(source, &temp).unwrap();
    assert!(key.install(&managed).is_err());
    assert!(temp.exists());
}

fn generate(path: &Path, kind: &str, passphrase: &str) {
    let mut command = Command::new("ssh-keygen");
    command.args(["-q", "-t", kind, "-N", passphrase]);
    if kind == "rsa" {
        command.args(["-b", "2048", "-m", "PEM"]);
    }
    assert!(
        command
            .arg("-f")
            .arg(path)
            .output()
            .unwrap()
            .status
            .success()
    );
}
