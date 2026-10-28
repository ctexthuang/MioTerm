use std::path::PathBuf;

use age::secrecy::{ExposeSecret, SecretString};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use url::Url;

use super::{Connections, Host};
use archive::Archive;

mod archive;
mod config;
#[cfg(test)]
mod credential_tests;
mod credentials;
mod crypto;
mod gitee;
pub use config::{Gitee, Provider, SyncSettings};
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
    pub gitee: Option<Gitee>,
    pub include_credentials: bool,
    pub remember_token: bool,
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
    pub credentials: usize,
}

impl Restored {
    pub fn apply(self, current: &mut Connections) -> anyhow::Result<()> {
        ensure!(
            current.hosts == self.before && current.next_id == self.before_next_id,
            "SSH sync: local hosts changed during synchronization; retry to include those changes"
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
                recent.host.remember_secret = host.remember_secret;
            }
        }
        current.validate()
    }
}

pub fn run(request: Request) -> anyhow::Result<Restored> {
    request.settings.validate()?;
    crypto::validate_passphrase(&request.passphrase)?;
    let remote = if let Some(settings) = &request.gitee {
        let token = if request.password.expose_secret().is_empty() && request.remember_token {
            SecretString::from(credentials::token()?.to_string())
        } else {
            request.password.clone()
        };
        Remote::Gitee(gitee::Client::new(settings, &token)?)
    } else {
        Remote::WebDav(webdav::Client::new(&request.settings, &request.password)?)
    };
    synchronize_with(request, remote, &credentials::SystemStore)
}

enum Remote {
    WebDav(webdav::Client),
    Gitee(gitee::Client),
}

#[cfg(test)]
fn synchronize(request: Request, remote: webdav::Client) -> anyhow::Result<Restored> {
    synchronize_with(request, Remote::WebDav(remote), &credentials::SystemStore)
}

fn synchronize_with(
    request: Request,
    remote: Remote,
    store: &impl credentials::Store,
) -> anyhow::Result<Restored> {
    let mut etag = None;
    let found = match &remote {
        Remote::WebDav(client) => client
            .get()?
            .map(|found| {
                etag = Some(found.etag);
                crypto::decrypt(&found.body, &request.passphrase)
            })
            .transpose()?,
        Remote::Gitee(client) => client.pull(&request.passphrase)?,
    };
    let mut archive = match found {
        Some(found) => found,
        None if request.restore_only => {
            anyhow::bail!("SSH sync: no encrypted archive exists in this vault or folder")
        }
        None => Archive::default(),
    };
    if !request.restore_only {
        let mut local = Archive::capture(&request.local)?;
        if request.include_credentials {
            local.capture_credentials(&request.local, store)?;
        }
        archive.merge(local)?;
        let encrypted = crypto::encrypt(&archive, &request.passphrase)?;
        match &remote {
            Remote::WebDav(client) => client.put(&encrypted, etag.as_deref())?,
            Remote::Gitee(client) => {
                client.put(&encrypted)?;
            }
        }
    }
    let hosts = archive.hosts.len();
    let keys = archive.keys.len();
    let before = request.local.hosts.clone();
    let before_next_id = request.local.next_id;
    let mut connections = archive.restore(request.local, &request.key_folder)?;
    let credentials = if request.include_credentials {
        archive.restore_credentials(&mut connections, store)?
    } else {
        0
    };
    Ok(Restored {
        before,
        before_next_id,
        connections,
        hosts,
        keys,
        credentials,
    })
}
