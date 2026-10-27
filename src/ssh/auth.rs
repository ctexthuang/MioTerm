use std::process::Command;

use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use super::Host;
use crate::term::Launch;

pub mod askpass;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Authentication {
    #[default]
    OpenSsh,
    Password,
    PrivateKey,
    Agent,
}

impl Authentication {
    pub const ALL: [Self; 4] = [Self::OpenSsh, Self::Password, Self::PrivateKey, Self::Agent];

    pub fn label(self) -> &'static str {
        match self {
            Self::OpenSsh => "OpenSSH config",
            Self::Password => "Password",
            Self::PrivateKey => "Private key",
            Self::Agent => "SSH Agent",
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::OpenSsh => "openssh",
            Self::Password => "password",
            Self::PrivateKey => "private_key",
            Self::Agent => "agent",
        }
    }

    pub fn has_secret(self) -> bool {
        matches!(self, Self::Password | Self::PrivateKey)
    }

    pub fn options(self) -> Vec<String> {
        let options: &[&str] = match self {
            Self::OpenSsh => &[],
            Self::Password => &[
                "PreferredAuthentications=password,keyboard-interactive",
                "PubkeyAuthentication=no",
            ],
            Self::PrivateKey => &[
                "PreferredAuthentications=publickey",
                "IdentitiesOnly=yes",
                "PasswordAuthentication=no",
                "KbdInteractiveAuthentication=no",
            ],
            Self::Agent => &[
                "PreferredAuthentications=publickey",
                "IdentityFile=none",
                "PasswordAuthentication=no",
                "KbdInteractiveAuthentication=no",
            ],
        };
        options
            .iter()
            .flat_map(|value| ["-o".into(), (*value).into()])
            .collect()
    }
}

pub fn account(host: &Host) -> String {
    let fields = serde_json::to_vec(&(
        &host.address,
        host.port,
        &host.user,
        host.auth,
        &host.identity,
    ))
    .expect("ssh: serializable credential identifier");
    format!("{:x}", Sha256::digest(fields))
}

fn entry(host: &Host) -> anyhow::Result<keyring::Entry> {
    keyring::Entry::new("com.zacharyzhang.elyterminal.ssh", &account(host))
        .context("ssh: cannot open the system credential store")
}

pub fn load(host: &Host) -> anyhow::Result<Option<Zeroizing<String>>> {
    match entry(host)?.get_password() {
        Ok(secret) => Ok(Some(Zeroizing::new(secret))),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(error).context("ssh: cannot read the system credential store"),
    }
}

pub fn forget(host: &Host) -> anyhow::Result<()> {
    match entry(host)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(error).context("ssh: cannot remove the saved credential"),
    }
}

pub fn save(host: &Host, secret: &str) -> anyhow::Result<()> {
    if !host.remember_secret {
        return forget(host);
    }
    if secret.is_empty() {
        ensure!(
            host.auth != Authentication::Password || load(host)?.is_some(),
            "ssh: enter a password to save in the system credential store"
        );
        return Ok(());
    }
    ensure!(
        !secret.contains(['\0', '\r', '\n']),
        "ssh: credential contains a line break or NUL"
    );
    ensure!(
        secret.len() <= 1023,
        "ssh: credential exceeds OpenSSH askpass's 1023-byte limit"
    );
    entry(host)?
        .set_password(secret)
        .context("ssh: cannot save to the system credential store")?;
    log::info!(
        "ssh: credential saved in the system store for {}",
        host.destination()
    );
    Ok(())
}

fn environment(host: &Host) -> anyhow::Result<Vec<(String, String)>> {
    if !host.remember_secret {
        return Ok(Vec::new());
    }
    let executable = std::env::current_exe().context("ssh: cannot locate the askpass helper")?;
    let path = executable
        .to_str()
        .context("ssh: askpass executable path is not UTF-8")?;
    Ok(vec![
        ("SSH_ASKPASS".into(), path.into()),
        ("SSH_ASKPASS_REQUIRE".into(), "force".into()),
        ("ELY_SSH_ASKPASS_ACCOUNT".into(), account(host)),
        ("ELY_SSH_ASKPASS_KIND".into(), host.auth.id().into()),
    ])
}

pub fn configure_launch(host: &Host, launch: &mut Launch) -> anyhow::Result<()> {
    launch.env.extend(environment(host)?);
    Ok(())
}

pub fn configure_command(host: &Host, command: &mut Command) -> anyhow::Result<()> {
    command.env("SSH_ASKPASS_REQUIRE", "never");
    command.envs(environment(host)?);
    Ok(())
}

#[cfg(test)]
mod tests;
