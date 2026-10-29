use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
    time::{Duration, Instant},
};

use anyhow::{Context as _, ensure};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use zeroize::{Zeroize, Zeroizing};

use super::private::{folder as secure_folder, protect, regular};

const KEY_LIMIT: usize = 65536;

pub(super) fn folder() -> anyhow::Result<std::path::PathBuf> {
    Ok(crate::settings::folder()?.join("keys"))
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Key {
    private: String,
    certificate: Option<String>,
}

impl Drop for Key {
    fn drop(&mut self) {
        self.private.zeroize();
        self.certificate.zeroize();
    }
}

impl Key {
    pub fn same_private(&self, other: &Self) -> bool {
        self.private == other.private
    }

    pub fn capture(path: &Path) -> anyhow::Result<Self> {
        let bytes = read(path)?;
        let mut certificate = path.as_os_str().to_owned();
        certificate.push("-cert.pub");
        let certificate = Path::new(&certificate);
        let certificate = match fs::symlink_metadata(certificate) {
            Ok(_) => Some(STANDARD.encode(read(certificate)?)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => {
                return Err(error).context("SSH keys: cannot inspect the SSH certificate");
            }
        };
        let key = Self {
            private: STANDARD.encode(&bytes),
            certificate,
        };
        key.validate()?;
        Ok(key)
    }

    fn decode(&self) -> anyhow::Result<Zeroizing<Vec<u8>>> {
        STANDARD
            .decode(&self.private)
            .map(Zeroizing::new)
            .map_err(|_| anyhow::anyhow!("SSH keys: invalid private key encoding"))
    }

    fn certificate(&self) -> anyhow::Result<Option<Zeroizing<Vec<u8>>>> {
        self.certificate
            .as_ref()
            .map(|value| {
                STANDARD
                    .decode(value)
                    .map(Zeroizing::new)
                    .map_err(|_| anyhow::anyhow!("SSH keys: invalid SSH certificate encoding"))
            })
            .transpose()
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            self.private.len() <= KEY_LIMIT * 2,
            "SSH keys: private key is too large"
        );
        let private = self.decode()?;
        ensure!(
            !private.is_empty() && private.len() <= KEY_LIMIT,
            "SSH keys: invalid private key size"
        );
        let text = std::str::from_utf8(&private)
            .map_err(|_| anyhow::anyhow!("SSH keys: identity is not a text SSH private key"))?;
        ensure!(
            ["OPENSSH", "RSA", "EC", "DSA", "ENCRYPTED", ""]
                .iter()
                .any(|kind| {
                    let name = if kind.is_empty() {
                        "PRIVATE KEY".into()
                    } else {
                        format!("{kind} PRIVATE KEY")
                    };
                    text.trim().starts_with(&format!("-----BEGIN {name}-----"))
                        && text.trim().ends_with(&format!("-----END {name}-----"))
                }),
            "SSH keys: identity must be an OpenSSH or PEM private key, not a public key or agent reference"
        );
        if let Some(certificate) = &self.certificate {
            ensure!(
                certificate.len() <= KEY_LIMIT * 2,
                "SSH keys: SSH certificate is too large"
            );
            let certificate = self.certificate()?.expect("the certificate is present");
            ensure!(
                certificate.len() <= KEY_LIMIT
                    && certificate
                        .split(|byte| *byte == b' ')
                        .next()
                        .is_some_and(|kind| kind.ends_with(b"-cert-v01@openssh.com")),
                "SSH keys: invalid SSH certificate"
            );
        }
        Ok(())
    }

    pub fn id(&self) -> String {
        let mut hash = Sha256::new();
        hash.update((self.private.len() as u64).to_be_bytes());
        hash.update(self.private.as_bytes());
        if let Some(certificate) = &self.certificate {
            hash.update(certificate.as_bytes());
        }
        format!("{:x}", hash.finalize())
    }

    pub fn install(&self, folder: &Path) -> anyhow::Result<String> {
        self.validate()?;
        secure_folder(folder)?;
        let _lock = lock(folder)?;
        let path = folder.join(self.id());
        let identity = path
            .to_str()
            .context("SSH keys: managed key path is not UTF-8")?;
        write(&path, &self.decode()?)?;
        if let Some(certificate) = self.certificate()? {
            let mut name = path.as_os_str().to_owned();
            name.push("-cert.pub");
            write(Path::new(&name), &certificate)?;
        }
        Ok(identity.to_owned())
    }
}

fn lock(folder: &Path) -> anyhow::Result<File> {
    let path = folder.join(".lock");
    match fs::symlink_metadata(&path) {
        Ok(_) => regular(&path, false)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).context("SSH keys: cannot inspect storage lock"),
    }
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let file = options
        .open(&path)
        .context("SSH keys: cannot open storage lock")?;
    regular(&path, false)?;
    protect(&path, false)?;
    let started = Instant::now();
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(file),
            Err(std::fs::TryLockError::WouldBlock)
                if started.elapsed() < Duration::from_secs(10) =>
            {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(error) => return Err(error).context("SSH keys: cannot acquire storage lock"),
        }
    }
}

fn read(path: &Path) -> anyhow::Result<Zeroizing<Vec<u8>>> {
    ensure!(
        fs::metadata(path)
            .with_context(|| format!(
                "SSH keys: cannot inspect referenced SSH key {}",
                path.display()
            ))?
            .is_file(),
        "SSH keys: SSH key is not a regular file"
    );
    let file = File::open(path).with_context(|| {
        format!(
            "SSH keys: cannot read referenced SSH key {}",
            path.display()
        )
    })?;
    ensure!(
        file.metadata()?.is_file(),
        "SSH keys: SSH key is not a regular file"
    );
    let mut bytes = Zeroizing::new(Vec::new());
    file.take(KEY_LIMIT as u64 + 1)
        .read_to_end(&mut bytes)
        .context("SSH keys: cannot read SSH key contents")?;
    ensure!(bytes.len() <= KEY_LIMIT, "SSH keys: SSH key exceeds 64 KiB");
    Ok(bytes)
}

fn write(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(_) => {
            regular(path, false)?;
            ensure!(
                read(path)?.as_slice() == bytes,
                "SSH keys: managed key already exists with different contents"
            );
            return protect(path, false);
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).context("SSH keys: cannot inspect managed key"),
    }
    let temp = path.with_extension("tmp");
    match fs::symlink_metadata(&temp) {
        Ok(_) => {
            regular(&temp, false)?;
            fs::remove_file(&temp).context("SSH keys: cannot remove interrupted key import")?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).context("SSH keys: cannot inspect temporary key"),
    }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&temp)
        .context("SSH keys: cannot create managed key")?;
    let saved = (|| {
        protect(&temp, false)?;
        file.write_all(bytes)
            .context("SSH keys: cannot write managed key")?;
        file.sync_all()
            .context("SSH keys: cannot flush managed key")?;
        drop(file);
        fs::rename(&temp, path).context("SSH keys: cannot install complete managed key")
    })();
    if saved.is_err()
        && let Err(error) = fs::remove_file(&temp)
    {
        log::error!("SSH keys: cannot remove incomplete managed key: {error}");
    }
    saved
}
