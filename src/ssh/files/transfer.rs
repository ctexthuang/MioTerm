use std::{path::PathBuf, process::Command, sync::atomic::AtomicBool, time::Duration};

use anyhow::{Context as _, ensure};

use super::{Remote, RemotePath};
use crate::ssh::remote::process;

#[derive(Clone)]
pub struct TransferItem {
    pub local: PathBuf,
    pub remote: RemotePath,
    pub directory: bool,
}

#[derive(Clone)]
pub struct Transfer {
    pub connection: Remote,
    pub upload: bool,
    pub items: Vec<TransferItem>,
}

fn argument(path: &[u8]) -> anyhow::Result<Vec<u8>> {
    ensure!(
        !path.is_empty() && !path.iter().any(|byte| *byte < 32 || *byte == 127),
        "sftp: path contains a control character; batch transfer refused"
    );
    let mut out = vec![b'"'];
    for &byte in path {
        if matches!(byte, b'"' | b'\\') {
            out.push(b'\\');
        }
        out.push(byte);
    }
    out.push(b'"');
    Ok(out)
}

impl Transfer {
    pub fn batch(&self) -> anyhow::Result<Vec<u8>> {
        ensure!(!self.items.is_empty(), "sftp: no file selected");
        let mut batch = Vec::new();
        for item in &self.items {
            ensure!(
                item.local.is_absolute() && item.remote.0.starts_with(b"/"),
                "sftp: transfer paths must be absolute"
            );
            let local_path = if item.directory && !self.upload {
                item.local
                    .parent()
                    .context("sftp: local folder has no parent")?
            } else {
                item.local.as_path()
            };
            let remote_path = if item.directory && self.upload {
                item.remote.parent()?
            } else {
                item.remote.clone()
            };
            let local = local_path.as_os_str().as_encoded_bytes();
            #[cfg(windows)]
            let local = local
                .iter()
                .map(|byte| if *byte == b'\\' { b'/' } else { *byte })
                .collect::<Vec<_>>();
            #[cfg(windows)]
            let local = local.as_slice();
            let (source, destination) = if self.upload {
                (local, remote_path.0.as_slice())
            } else {
                (remote_path.0.as_slice(), local)
            };
            batch.extend_from_slice(if self.upload { b"@put " } else { b"@get " });
            if item.directory {
                batch.extend_from_slice(b"-R ");
            }
            batch.extend(argument(source)?);
            batch.push(b' ');
            batch.extend(argument(destination)?);
            batch.push(b'\n');
        }
        batch.extend_from_slice(b"bye\n");
        Ok(batch)
    }

    pub fn command(&self) -> anyhow::Result<Command> {
        self.connection.host.validate()?;
        let program = crate::settings::program_path("sftp")
            .context("sftp: system OpenSSH SFTP client is unavailable; install OpenSSH Client")?;
        let mut command = Command::new(program);
        command.args(self.connection.options());
        command.args(["-b", "-", "-P", &self.connection.host.port.to_string()]);
        let host = &self.connection.host;
        command.args(host.auth.options());
        if !host.user.is_empty() {
            command.args(["-o", &format!("User={}", host.user)]);
        }
        if !host.identity.is_empty() {
            command.arg("-i").arg(&host.identity);
        }
        let destination = if host.address.contains(':') {
            format!("[{}]", host.address)
        } else {
            host.address.clone()
        };
        command.arg("--").arg(destination);
        super::super::auth::configure_command(host, &mut command)?;
        #[cfg(windows)]
        std::os::windows::process::CommandExt::creation_flags(
            &mut command,
            crate::git::CREATE_NO_WINDOW,
        );
        Ok(command)
    }

    pub fn run(&self, cancel: &AtomicBool) -> anyhow::Result<()> {
        let batch = self.batch()?;
        self.connection.wait(cancel)?;
        log::info!(
            "sftp: {} {} items on {}",
            if self.upload {
                "uploading"
            } else {
                "downloading"
            },
            self.items.len(),
            self.connection.host.destination()
        );
        let out = process::output(
            &mut self.command()?,
            &batch,
            cancel,
            Duration::from_secs(24 * 60 * 60),
        )
        .context("sftp: transfer stopped; partial files may remain")?;
        ensure!(
            out.status.success(),
            "sftp: transfer failed; partial files may remain: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
        log::info!(
            "sftp: transfer finished on {}",
            self.connection.host.destination()
        );
        Ok(())
    }
}
