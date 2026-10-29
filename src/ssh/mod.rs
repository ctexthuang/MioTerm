use std::path::{Path, PathBuf};

use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};

use crate::term::{Launch, Quote};

pub mod auth;
pub mod control;
pub mod files;
pub mod panel;
mod private;
pub mod remote;
pub mod shell;
mod store;
pub mod sync;
#[cfg(test)]
mod tests;
pub(crate) mod vault;

pub use auth::Authentication;
pub use store::HostStore;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Host {
    pub id: u64,
    pub name: String,
    pub address: String,
    pub port: u16,
    pub user: String,
    pub group: String,
    pub identity: String,
    #[serde(default)]
    pub auth: Authentication,
    #[serde(default)]
    pub remember_secret: bool,
}

impl Default for Host {
    fn default() -> Self {
        Self {
            id: 0,
            name: String::new(),
            address: "127.0.0.1".into(),
            port: 22,
            user: String::new(),
            group: String::new(),
            identity: String::new(),
            auth: Authentication::default(),
            remember_secret: false,
        }
    }
}

impl Host {
    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            self.auth != Authentication::PrivateKey || !self.identity.is_empty(),
            "ssh: private key authentication requires an identity file"
        );
        ensure!(
            !self.remember_secret || self.auth.has_secret(),
            "ssh: only password and private key modes can save a credential"
        );
        ensure!(
            !self.address.is_empty()
                && !self.address.starts_with('-')
                && self
                    .address
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ':' | '%')),
            "ssh: host must be a hostname, SSH alias, IPv4 or IPv6 address"
        );
        ensure!(self.port > 0, "ssh: port must be between 1 and 65535");
        ensure!(
            !self.user.starts_with('-')
                && !self
                    .user
                    .chars()
                    .any(|c| c.is_whitespace() || c.is_control()),
            "ssh: username must not start with '-' or contain whitespace or controls"
        );
        for (field, value) in [
            ("name", &self.name),
            ("group", &self.group),
            ("identity path", &self.identity),
        ] {
            ensure!(
                !value.chars().any(char::is_control),
                "ssh: {field} contains a control character"
            );
        }
        if !self.identity.is_empty() {
            ensure!(
                Path::new(&self.identity).is_absolute(),
                "ssh: identity path must be absolute"
            );
        }
        Ok(())
    }

    pub fn label(&self) -> &str {
        if self.name.is_empty() {
            &self.address
        } else {
            &self.name
        }
    }

    pub fn destination(&self) -> String {
        let address = if self.address.contains(':') {
            format!("[{}]", self.address)
        } else {
            self.address.clone()
        };
        let user = if self.user.is_empty() {
            String::new()
        } else {
            format!("{}@", self.user)
        };
        format!("{user}{address}:{}", self.port)
    }

    pub fn matches(&self, query: &str) -> bool {
        [&self.name, &self.address, &self.user, &self.group]
            .iter()
            .any(|value| value.to_lowercase().contains(query))
    }

    fn same_target(&self, other: &Self) -> bool {
        (
            &self.address,
            self.port,
            &self.user,
            &self.identity,
            self.auth,
        ) == (
            &other.address,
            other.port,
            &other.user,
            &other.identity,
            other.auth,
        )
    }

    fn arguments(&self) -> anyhow::Result<Vec<String>> {
        self.validate()?;
        let mut args = vec!["-tt".into(), "-p".into(), self.port.to_string()];
        args.extend(self.auth.options());
        if !self.user.is_empty() {
            args.extend(["-l".into(), self.user.clone()]);
        }
        if !self.identity.is_empty() {
            args.extend(["-i".into(), self.identity.clone()]);
        }
        args.extend(["--".into(), self.address.clone()]);
        Ok(args)
    }

    pub fn launch(&self, cwd: PathBuf) -> anyhow::Result<Launch> {
        let args = self.arguments()?;
        if !self.identity.is_empty() {
            ensure!(
                Path::new(&self.identity).is_file(),
                "ssh: identity file does not exist: {}",
                self.identity
            );
        }
        let program = crate::settings::program_path("ssh")
            .context("ssh: system OpenSSH client is unavailable; install OpenSSH Client")?;
        let mut launch = Launch {
            program: Some((program, args)),
            cwd,
            env: Vec::new(),
            reports: false,
            quote: Quote::Posix,
        };
        auth::configure_launch(self, &mut launch)?;
        Ok(launch)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recent {
    pub host: Host,
    pub opened_at: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Connections {
    pub next_id: u64,
    pub hosts: Vec<Host>,
    pub recent: Vec<Recent>,
}

impl Default for Connections {
    fn default() -> Self {
        Self {
            next_id: 1,
            hosts: Vec::new(),
            recent: Vec::new(),
        }
    }
}

impl Connections {
    pub fn validate(&self) -> anyhow::Result<()> {
        let mut ids = std::collections::HashSet::new();
        for host in &self.hosts {
            host.validate()?;
            ensure!(
                host.id > 0 && host.id < self.next_id && ids.insert(host.id),
                "ssh: duplicate, zero or unallocated host id"
            );
        }
        ensure!(self.next_id > 0, "ssh: host ids exhausted");
        ensure!(
            self.recent.len() <= 20,
            "ssh: more than 20 recent connections"
        );
        for recent in &self.recent {
            recent.host.validate()?;
        }
        Ok(())
    }

    pub fn upsert(&mut self, mut host: Host) -> anyhow::Result<Host> {
        host.validate()?;
        if host.id == 0 {
            host.id = self.next_id;
            self.next_id = self
                .next_id
                .checked_add(1)
                .context("ssh: host ids exhausted")?;
            self.hosts.push(host.clone());
        } else {
            let saved = self
                .hosts
                .iter_mut()
                .find(|saved| saved.id == host.id)
                .context("ssh: the host being edited no longer exists")?;
            *saved = host.clone();
        }
        Ok(host)
    }

    pub fn opened(&mut self, host: Host, opened_at: u64) {
        self.recent.retain(|recent| !recent.host.same_target(&host));
        self.recent.insert(0, Recent { host, opened_at });
        self.recent.truncate(20);
    }
}
