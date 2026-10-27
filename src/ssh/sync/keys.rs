use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
};

use anyhow::{Context as _, ensure};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use zeroize::{Zeroize, Zeroizing};

#[cfg(windows)]
mod windows;

const KEY_LIMIT: usize = 65536;

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
    pub fn capture(path: &Path) -> anyhow::Result<Self> {
        let bytes = read(path)?;
        let mut certificate = path.as_os_str().to_owned();
        certificate.push("-cert.pub");
        let certificate = Path::new(&certificate);
        let certificate = match fs::symlink_metadata(certificate) {
            Ok(_) => Some(STANDARD.encode(read(certificate)?)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error).context("WebDAV: cannot inspect the SSH certificate"),
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
            .map_err(|_| anyhow::anyhow!("WebDAV: invalid private key encoding"))
    }

    fn certificate(&self) -> anyhow::Result<Option<Zeroizing<Vec<u8>>>> {
        self.certificate
            .as_ref()
            .map(|value| {
                STANDARD
                    .decode(value)
                    .map(Zeroizing::new)
                    .map_err(|_| anyhow::anyhow!("WebDAV: invalid SSH certificate encoding"))
            })
            .transpose()
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            self.private.len() <= KEY_LIMIT * 2,
            "WebDAV: private key is too large"
        );
        let private = self.decode()?;
        ensure!(
            !private.is_empty() && private.len() <= KEY_LIMIT,
            "WebDAV: invalid private key size"
        );
        let text = std::str::from_utf8(&private)
            .map_err(|_| anyhow::anyhow!("WebDAV: identity is not a text SSH private key"))?;
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
            "WebDAV: identity must be an OpenSSH or PEM private key, not a public key or agent reference"
        );
        if let Some(certificate) = &self.certificate {
            ensure!(
                certificate.len() <= KEY_LIMIT * 2,
                "WebDAV: SSH certificate is too large"
            );
            let certificate = self.certificate()?.expect("the certificate is present");
            ensure!(
                certificate.len() <= KEY_LIMIT
                    && certificate
                        .split(|byte| *byte == b' ')
                        .next()
                        .is_some_and(|kind| kind.ends_with(b"-cert-v01@openssh.com")),
                "WebDAV: invalid SSH certificate"
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
        let path = folder.join(self.id());
        write(&path, &self.decode()?)?;
        if let Some(certificate) = self.certificate()? {
            let mut name = path.as_os_str().to_owned();
            name.push("-cert.pub");
            write(Path::new(&name), &certificate)?;
        }
        path.to_str()
            .map(str::to_owned)
            .context("WebDAV: restored key path is not UTF-8")
    }
}

fn read(path: &Path) -> anyhow::Result<Zeroizing<Vec<u8>>> {
    ensure!(
        fs::metadata(path)
            .with_context(|| format!(
                "WebDAV: cannot inspect referenced SSH key {}",
                path.display()
            ))?
            .is_file(),
        "WebDAV: SSH key is not a regular file"
    );
    let file = File::open(path)
        .with_context(|| format!("WebDAV: cannot read referenced SSH key {}", path.display()))?;
    ensure!(
        file.metadata()?.is_file(),
        "WebDAV: SSH key is not a regular file"
    );
    let mut bytes = Zeroizing::new(Vec::new());
    file.take(KEY_LIMIT as u64 + 1)
        .read_to_end(&mut bytes)
        .context("WebDAV: cannot read SSH key contents")?;
    ensure!(bytes.len() <= KEY_LIMIT, "WebDAV: SSH key exceeds 64 KiB");
    Ok(bytes)
}

fn regular(path: &Path, folder: bool) -> anyhow::Result<()> {
    let metadata = fs::symlink_metadata(path).context("WebDAV: cannot inspect managed key path")?;
    ensure!(
        !metadata.file_type().is_symlink()
            && if folder {
                metadata.is_dir()
            } else {
                metadata.is_file()
            },
        "WebDAV: managed key path is a symlink or has the wrong type"
    );
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        ensure!(
            metadata.file_attributes() & 0x400 == 0,
            "WebDAV: managed key path is a reparse point"
        );
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        ensure!(
            metadata.uid() == unsafe { libc::geteuid() },
            "WebDAV: managed key path has another owner"
        );
    }
    Ok(())
}

fn secure_folder(path: &Path) -> anyhow::Result<()> {
    if !path
        .try_exists()
        .context("WebDAV: cannot inspect managed key folder")?
    {
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(path)
                .context("WebDAV: cannot create private key folder")?;
        }
        #[cfg(not(unix))]
        fs::create_dir_all(path).context("WebDAV: cannot create private key folder")?;
    }
    regular(path, true)?;
    protect(path, true)
}

fn write(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    if path
        .try_exists()
        .context("WebDAV: cannot inspect restored key")?
    {
        regular(path, false)?;
        ensure!(
            read(path)?.as_slice() == bytes,
            "WebDAV: restored key already exists with different contents"
        );
        return protect(path, false);
    }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .context("WebDAV: cannot create restored key")?;
    let saved = (|| {
        protect(path, false)?;
        file.write_all(bytes)
            .context("WebDAV: cannot write restored key")?;
        file.sync_all().context("WebDAV: cannot flush restored key")
    })();
    if saved.is_err() {
        drop(file);
        if let Err(error) = fs::remove_file(path) {
            log::error!("WebDAV: cannot remove incomplete restored key: {error}");
        }
    }
    saved
}

#[cfg(unix)]
fn protect(path: &Path, folder: bool) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(
        path,
        fs::Permissions::from_mode(if folder { 0o700 } else { 0o600 }),
    )
    .context("WebDAV: cannot restrict restored key permissions")
}

#[cfg(windows)]
fn protect(path: &Path, folder: bool) -> anyhow::Result<()> {
    windows::protect(path, folder)
}
