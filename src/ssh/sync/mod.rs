use std::path::PathBuf;

use age::secrecy::SecretString;
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use url::Url;

use super::{Connections, Host};
use archive::Archive;

mod archive;
mod crypto;
#[cfg(test)]
mod flow_tests;
mod keys;
pub mod page;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod transport_tests;
mod webdav;

pub(super) const LIMIT: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WebDav {
    pub url: String,
    pub username: String,
}

impl WebDav {
    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            !self.username.contains(':') && !self.username.chars().any(char::is_control),
            "WebDAV: username contains a colon or control character"
        );
        if !self.url.is_empty() {
            self.endpoint()?;
        }
        Ok(())
    }

    fn endpoint(&self) -> anyhow::Result<Url> {
        let mut url = Url::parse(&self.url).context("WebDAV: invalid server folder URL")?;
        ensure!(
            url.scheme() == "https" && url.host_str().is_some(),
            "WebDAV: an HTTPS server folder URL is required"
        );
        ensure!(
            url.username().is_empty() && url.password().is_none(),
            "WebDAV: put credentials in their fields, not in the URL"
        );
        ensure!(
            url.query().is_none() && url.fragment().is_none(),
            "WebDAV: the folder URL must not contain a query or fragment"
        );
        url.path_segments_mut()
            .map_err(|_| anyhow::anyhow!("WebDAV: URL has no folder path"))?
            .pop_if_empty()
            .push("elyterminal-ssh.age");
        Ok(url)
    }
}

pub struct Request {
    pub settings: WebDav,
    pub password: SecretString,
    pub passphrase: SecretString,
    pub local: Connections,
    pub key_folder: PathBuf,
    pub restore_only: bool,
}

pub struct Restored {
    pub before: Vec<Host>,
    pub before_next_id: u64,
    pub connections: Connections,
    pub hosts: usize,
    pub keys: usize,
}

impl Restored {
    pub fn apply(self, current: &mut Connections) -> anyhow::Result<()> {
        ensure!(
            current.hosts == self.before && current.next_id == self.before_next_id,
            "WebDAV: local hosts changed during synchronization; retry to include those changes"
        );
        current.hosts = self.connections.hosts;
        current.next_id = self.connections.next_id;
        for recent in &mut current.recent {
            if self
                .before
                .iter()
                .any(|host| host.id == recent.host.id && host.same_target(&recent.host))
                && let Some(host) = current.hosts.iter().find(|host| host.id == recent.host.id)
            {
                recent.host.identity = host.identity.clone();
            }
        }
        current.validate()
    }
}

pub fn run(request: Request) -> anyhow::Result<Restored> {
    request.settings.validate()?;
    crypto::validate_passphrase(&request.passphrase)?;
    let remote = webdav::Client::new(&request.settings, &request.password)?;
    synchronize(request, remote)
}

fn synchronize(request: Request, remote: webdav::Client) -> anyhow::Result<Restored> {
    let found = remote.get()?;
    let mut archive = match &found {
        Some(found) => crypto::decrypt(&found.body, &request.passphrase)?,
        None if request.restore_only => {
            anyhow::bail!("WebDAV: no SSH archive exists in this folder")
        }
        None => Archive::default(),
    };
    if !request.restore_only {
        archive.merge(Archive::capture(&request.local)?)?;
        let encrypted = crypto::encrypt(&archive, &request.passphrase)?;
        remote.put(&encrypted, found.as_ref().map(|found| found.etag.as_str()))?;
    }
    let hosts = archive.hosts.len();
    let keys = archive.keys.len();
    let before = request.local.hosts.clone();
    let before_next_id = request.local.next_id;
    let connections = archive.restore(request.local, &request.key_folder)?;
    Ok(Restored {
        before,
        before_next_id,
        connections,
        hosts,
        keys,
    })
}
