use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

use age::secrecy::ExposeSecret;
use anyhow::{Context as _, ensure};
use zeroize::Zeroizing;

use super::{Contents, LIMIT, Vault};
use crate::ssh::private;

static WRITES: AtomicU64 = AtomicU64::new(0);

fn exists(path: &Path) -> anyhow::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error).context("Credential vault: cannot inspect private file"),
    }
}

fn open(path: &Path, create: bool) -> anyhow::Result<File> {
    if exists(path)? {
        private::regular(path, false)?;
        private::protect(path, false)?;
    }
    let mut options = OpenOptions::new();
    options.read(true).write(create).create(create);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let file = options
        .open(path)
        .context("Credential vault: cannot open private file")?;
    private::regular(path, false)?;
    private::protect(path, false)?;
    Ok(file)
}

fn read(path: &Path, limit: usize) -> anyhow::Result<Zeroizing<Vec<u8>>> {
    let file = open(path, false)?;
    let mut bytes = Zeroizing::new(Vec::new());
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .context("Credential vault: cannot read private file")?;
    ensure!(
        bytes.len() <= limit,
        "Credential vault: private file exceeds the size limit"
    );
    Ok(bytes)
}

fn replace(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    if exists(path)? {
        private::regular(path, false)?;
    }
    let temp = path.with_extension(format!(
        "{}-{}.tmp",
        std::process::id(),
        WRITES.fetch_add(1, Ordering::Relaxed)
    ));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&temp)
        .context("Credential vault: cannot create temporary file")?;
    let result = (|| {
        private::protect(&temp, false)?;
        file.write_all(bytes)
            .context("Credential vault: cannot write private file")?;
        file.sync_all()
            .context("Credential vault: cannot flush private file")?;
        drop(file);
        fs::rename(&temp, path).context("Credential vault: cannot replace private file")?;
        #[cfg(unix)]
        File::open(path.parent().expect("vault folder"))?
            .sync_all()
            .context("Credential vault: cannot flush private folder")?;
        Ok(())
    })();
    if result.is_err()
        && exists(&temp)?
        && let Err(error) = fs::remove_file(&temp)
    {
        log::error!("Credential vault: cannot remove incomplete temporary file: {error}");
    }
    result
}

impl Vault {
    fn lock(&self) -> anyhow::Result<File> {
        private::folder(&self.folder)?;
        let file = open(&self.folder.join(".lock"), true)?;
        let started = Instant::now();
        loop {
            match file.try_lock() {
                Ok(()) => return Ok(file),
                Err(std::fs::TryLockError::WouldBlock)
                    if started.elapsed() < Duration::from_secs(10) =>
                {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => {
                    return Err(error).context("Credential vault: cannot acquire storage lock");
                }
            }
        }
    }

    fn identity(&self) -> anyhow::Result<age::x25519::Identity> {
        let path = self.folder.join("local.key");
        if !exists(&path)? {
            ensure!(
                !exists(&self.folder.join("credentials.age"))?,
                "Credential vault: local.key is missing; restore the original key, do not replace the vault"
            );
            let identity = age::x25519::Identity::generate();
            replace(&path, identity.to_string().expose_secret().as_bytes())?;
            log::info!("Credential vault: local encryption key created");
            return Ok(identity);
        }
        let bytes = read(&path, 1024)?;
        let text = std::str::from_utf8(&bytes)
            .context("Credential vault: invalid local encryption key")?;
        text.parse()
            .map_err(|_| anyhow::anyhow!("Credential vault: invalid local encryption key"))
    }

    fn contents(&self, identity: &age::x25519::Identity) -> anyhow::Result<Contents> {
        let path = self.folder.join("credentials.age");
        if !exists(&path)? {
            return Ok(Contents::default());
        }
        let bytes = read(&path, LIMIT + 65536)?;
        let decryptor = age::Decryptor::new(bytes.as_slice())
            .context("Credential vault: invalid encrypted file")?;
        let reader = decryptor
            .decrypt(std::iter::once(identity as &dyn age::Identity))
            .context("Credential vault: wrong local key or damaged file")?;
        let mut plain = Zeroizing::new(Vec::new());
        reader
            .take(LIMIT as u64 + 1)
            .read_to_end(&mut plain)
            .context("Credential vault: encrypted file is damaged or incomplete")?;
        ensure!(
            plain.len() <= LIMIT,
            "Credential vault: decrypted file exceeds 8 MiB"
        );
        serde_json::from_slice(&plain)
            .map_err(|_| anyhow::anyhow!("Credential vault: invalid decrypted contents"))
    }

    pub(super) fn read<T>(&self, use_contents: impl FnOnce(&Contents) -> T) -> anyhow::Result<T> {
        let _lock = self.lock()?;
        let identity = self.identity()?;
        Ok(use_contents(&self.contents(&identity)?))
    }

    pub(super) fn edit<T>(
        &self,
        edit: impl FnOnce(&mut Contents) -> anyhow::Result<T>,
    ) -> anyhow::Result<T> {
        let _lock = self.lock()?;
        let identity = self.identity()?;
        let mut contents = self.contents(&identity)?;
        let result = edit(&mut contents)?;
        let plain = Zeroizing::new(
            serde_json::to_vec(&contents).context("Credential vault: cannot encode contents")?,
        );
        ensure!(
            plain.len() <= LIMIT,
            "Credential vault: contents exceed 8 MiB"
        );
        let recipient = identity.to_public();
        let encryptor =
            age::Encryptor::with_recipients(std::iter::once(&recipient as &dyn age::Recipient))
                .context("Credential vault: cannot initialize encryption")?;
        let mut bytes = Vec::new();
        let mut writer = encryptor
            .wrap_output(&mut bytes)
            .context("Credential vault: cannot start encryption")?;
        writer
            .write_all(&plain)
            .context("Credential vault: encryption failed")?;
        writer
            .finish()
            .context("Credential vault: encryption did not finish")?;
        replace(&self.folder.join("credentials.age"), &bytes)?;
        Ok(result)
    }
}
