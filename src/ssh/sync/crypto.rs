use std::io::{Read, Write};

use age::secrecy::{ExposeSecret, SecretString};
use anyhow::{Context as _, ensure};
use zeroize::Zeroizing;

use super::{LIMIT, archive::Archive};

pub(super) fn validate_passphrase(passphrase: &SecretString) -> anyhow::Result<()> {
    ensure!(
        !passphrase.expose_secret().is_empty(),
        "SSH sync: enter the encryption passphrase or enable remembrance and save it first"
    );
    ensure!(
        passphrase.expose_secret().chars().count() >= 6,
        "SSH sync: the encryption passphrase must have at least 6 characters"
    );
    Ok(())
}

pub(super) fn encrypt(archive: &Archive, passphrase: &SecretString) -> anyhow::Result<Vec<u8>> {
    validate_passphrase(passphrase)?;
    archive.validate()?;
    let plain =
        Zeroizing::new(serde_json::to_vec(archive).context("SSH sync: cannot encode archive")?);
    ensure!(plain.len() <= LIMIT, "SSH sync: SSH archive exceeds 8 MiB");
    let encryptor = age::Encryptor::with_user_passphrase(passphrase.clone());
    let mut encrypted = Vec::new();
    let mut writer = encryptor
        .wrap_output(&mut encrypted)
        .context("SSH sync: cannot start archive encryption")?;
    writer
        .write_all(&plain)
        .context("SSH sync: encryption failed")?;
    writer
        .finish()
        .context("SSH sync: encryption did not finish")?;
    Ok(encrypted)
}

pub(super) fn decrypt(ciphertext: &[u8], passphrase: &SecretString) -> anyhow::Result<Archive> {
    ensure!(
        ciphertext.len() <= LIMIT + 65536,
        "SSH sync: encrypted archive exceeds the size limit"
    );
    let decryptor = age::Decryptor::new(ciphertext)
        .context("SSH sync: the remote file is not a valid encrypted age archive")?;
    let mut identity = age::scrypt::Identity::new(passphrase.clone());
    identity.set_max_work_factor(20);
    let reader = decryptor
        .decrypt(std::iter::once(&identity as &dyn age::Identity))
        .context("SSH sync: wrong encryption passphrase or damaged archive")?;
    let mut plain = Zeroizing::new(Vec::new());
    reader
        .take(LIMIT as u64 + 1)
        .read_to_end(&mut plain)
        .context("SSH sync: encrypted archive is damaged or incomplete")?;
    ensure!(
        plain.len() <= LIMIT,
        "SSH sync: decrypted archive exceeds 8 MiB"
    );
    let archive: Archive = serde_json::from_slice(&plain)
        .map_err(|_| anyhow::anyhow!("SSH sync: invalid SSH archive contents"))?;
    archive.validate()?;
    Ok(archive)
}
