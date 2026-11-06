use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    io::Read,
    time::Duration,
};

use age::secrecy::{ExposeSecret, SecretString};
use anyhow::{Context as _, ensure};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::Deserialize;
use ureq::{Agent, http::HeaderValue};
use zeroize::Zeroizing;

use super::{Gitee, archive::Archive, credentials, crypto};

const FILE: &str = "mioterm-ssh-v2.age.base64";
const LEGACY_FILE: &str = "mioterm-ssh-v1.age.base64";
const SNAPSHOT_LIMIT: usize = 512 * 1024;
const RESPONSE_LIMIT: usize = 2 * 1024 * 1024;

pub(super) struct Client {
    agent: Agent,
    base: String,
    authorization: Zeroizing<String>,
    description: String,
    configured_id: String,
    target: RefCell<Option<Target>>,
}

#[derive(Clone)]
struct Target {
    id: String,
    content: String,
}

#[derive(Deserialize)]
struct Summary {
    id: String,
    public: bool,
    description: Option<String>,
    #[serde(default)]
    created_at: String,
}

#[derive(Deserialize)]
struct Snippet {
    id: String,
    public: bool,
    description: Option<String>,
    #[serde(default)]
    truncated: bool,
    #[serde(default)]
    files: BTreeMap<String, File>,
}

#[derive(Deserialize)]
struct File {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    truncated: bool,
}

pub(super) fn validate_id(id: &str) -> anyhow::Result<()> {
    ensure!(
        !id.is_empty()
            && id.len() <= 128
            && id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte)),
        "Gitee sync: invalid snippet identifier"
    );
    Ok(())
}

impl Client {
    pub fn new(settings: &Gitee, token: &SecretString) -> anyhow::Result<Self> {
        Self::at("https://gitee.com/api/v5", settings, token)
    }

    pub(super) fn at(base: &str, settings: &Gitee, token: &SecretString) -> anyhow::Result<Self> {
        settings.validate()?;
        credentials::validate_token(token.expose_secret())?;
        Ok(Self {
            agent: Agent::config_builder()
                .timeout_global(Some(Duration::from_secs(45)))
                .timeout_connect(Some(Duration::from_secs(10)))
                .max_redirects(0)
                .http_status_as_error(false)
                .build()
                .into(),
            base: base.into(),
            authorization: Zeroizing::new(format!("Bearer {}", token.expose_secret())),
            description: settings.description(),
            configured_id: settings.gist_id.clone(),
            target: RefCell::new(None),
        })
    }

    fn record_description(&self) -> String {
        format!("{} records", self.description)
    }

    pub fn target_id(&self) -> Option<String> {
        self.target
            .borrow()
            .as_ref()
            .map(|target| target.id.clone())
    }

    fn authorization(&self) -> HeaderValue {
        let mut header = HeaderValue::from_str(&self.authorization).expect("validated Gitee token");
        header.set_sensitive(true);
        header
    }

    fn read<T: serde::de::DeserializeOwned>(
        &self,
        mut response: ureq::http::Response<ureq::Body>,
        status: u16,
    ) -> anyhow::Result<T> {
        ensure!(
            response.status().as_u16() == status,
            "Gitee sync: HTTP {}; check the token, gists permission and API limits",
            response.status().as_u16()
        );
        let mut body = Vec::new();
        response
            .body_mut()
            .as_reader()
            .take(RESPONSE_LIMIT as u64 + 1)
            .read_to_end(&mut body)
            .map_err(|_| anyhow::anyhow!("Gitee sync: cannot read the API response"))?;
        ensure!(
            body.len() <= RESPONSE_LIMIT,
            "Gitee sync: API response exceeds the size limit"
        );
        serde_json::from_slice(&body)
            .map_err(|_| anyhow::anyhow!("Gitee sync: invalid API response"))
    }

    fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> anyhow::Result<T> {
        let response = self
            .agent
            .get(format!("{}{path}", self.base))
            .header("Authorization", self.authorization())
            .header("Accept", "application/json")
            .header("Accept-Encoding", "identity")
            .call()
            .map_err(|_| {
                anyhow::anyhow!("Gitee sync: request failed; check TLS and the network")
            })?;
        self.read(response, 200)
    }

    fn list(&self) -> anyhow::Result<Vec<Summary>> {
        let mut snippets = BTreeMap::new();
        for page in 1..=10 {
            let batch: Vec<Summary> = self.get(&format!("/gists?page={page}&per_page=100"))?;
            let count = batch.len();
            for snippet in batch {
                if snippet.description.as_deref() == Some(self.description.as_str())
                    || snippet.description.as_deref() == Some(self.record_description().as_str())
                {
                    validate_id(&snippet.id)?;
                    ensure!(
                        !snippet.public,
                        "Gitee sync: a vault snippet is public; make it private before syncing"
                    );
                    snippets.insert(snippet.id.clone(), snippet);
                }
            }
            if count < 100 {
                return Ok(snippets.into_values().collect());
            }
        }
        anyhow::bail!("Gitee sync: more than 1000 snippets; refusing incomplete synchronization")
    }

    fn content<'a>(&self, detail: &'a Snippet, id: &str, legacy: bool) -> anyhow::Result<&'a str> {
        let description = if legacy {
            self.description.clone()
        } else {
            self.record_description()
        };
        ensure!(
            detail.id == id
                && !detail.public
                && detail.description.as_deref() == Some(description.as_str()),
            "Gitee sync: snippet identity or privacy changed"
        );
        let file = detail
            .files
            .get(if legacy { LEGACY_FILE } else { FILE })
            .context("Gitee sync: encrypted archive is missing")?;
        ensure!(
            !detail.truncated && !file.truncated,
            "Gitee sync: archive was truncated; refusing incomplete data"
        );
        let content = file
            .content
            .as_deref()
            .context("Gitee sync: no inline archive; refusing external download URLs")?;
        ensure!(
            content.len() <= SNAPSHOT_LIMIT * 2,
            "Gitee sync: archive is too large"
        );
        Ok(content)
    }

    fn decode(&self, content: &str, passphrase: &SecretString) -> anyhow::Result<Archive> {
        let bytes = STANDARD
            .decode(content)
            .context("Gitee sync: invalid archive encoding")?;
        ensure!(
            bytes.len() <= SNAPSHOT_LIMIT,
            "Gitee sync: archive exceeds 512 KiB"
        );
        crypto::decrypt(&bytes, passphrase)
    }

    pub fn pull(&self, passphrase: &SecretString) -> anyhow::Result<Option<Archive>> {
        let mut snippets = if self.configured_id.is_empty() {
            self.list()?
        } else {
            Vec::new()
        };
        let records: Vec<_> = snippets
            .iter()
            .filter(|snippet| {
                snippet.description.as_deref() == Some(self.record_description().as_str())
            })
            .collect();
        ensure!(
            records.len() <= 1,
            "Gitee sync: multiple record vaults found; select sync.gitee.gist_id in settings.json"
        );
        let id = if !self.configured_id.is_empty() {
            Some(self.configured_id.clone())
        } else {
            records.first().map(|snippet| snippet.id.clone())
        };
        if let Some(id) = id {
            let detail: Snippet = self.get(&format!("/gists/{id}"))?;
            let content = self.content(&detail, &id, false)?;
            let archive = self.decode(content, passphrase)?;
            ensure!(
                archive.format == 2,
                "Gitee sync: record vault contains an obsolete archive"
            );
            *self.target.borrow_mut() = Some(Target {
                id,
                content: content.into(),
            });
            return Ok(Some(archive));
        }
        if snippets.is_empty() {
            return Ok(None);
        }
        snippets.sort_by(|left, right| {
            right
                .created_at
                .cmp(&left.created_at)
                .then(right.id.cmp(&left.id))
        });
        let mut archive = Archive::default();
        let mut seen = BTreeSet::new();
        let mut reads = 0;
        for snippet in snippets {
            if seen.contains(&snippet.id) {
                continue;
            }
            reads += 1;
            ensure!(
                reads <= 32,
                "Gitee sync: too many legacy snapshots; consolidate older vaults first"
            );
            let detail: Snippet = self.get(&format!("/gists/{}", snippet.id))?;
            let found = self.decode(self.content(&detail, &snippet.id, true)?, passphrase)?;
            seen.extend(found.snapshots.clone());
            archive.merge(found)?;
            seen.insert(snippet.id);
        }
        log::info!("Gitee sync: migrated legacy snapshots into record synchronization");
        Ok(Some(archive))
    }

    pub fn put(&self, encrypted: &[u8]) -> anyhow::Result<String> {
        ensure!(
            encrypted.len() <= SNAPSHOT_LIMIT,
            "Gitee sync: archive exceeds 512 KiB; use WebDAV for larger vaults"
        );
        let target = self.target.borrow().clone();
        if let Some(target) = &target {
            let current: Snippet = self.get(&format!("/gists/{}", target.id))?;
            ensure!(
                self.content(&current, &target.id, false)? == target.content,
                "Gitee sync: another device changed the archive; retry to merge its changes"
            );
        }
        let content = STANDARD.encode(encrypted);
        let payload = serde_json::json!({"public": false, "description": self.record_description(),
            "files": { FILE: { "content": content } }});
        let body = serde_json::to_vec(&payload).context("Gitee sync: cannot encode upload")?;
        let request = match &target {
            Some(target) => self
                .agent
                .patch(format!("{}/gists/{}", self.base, target.id)),
            None => self.agent.post(format!("{}/gists", self.base)),
        };
        let response = request
            .header("Authorization", self.authorization())
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .send(body.as_slice())
            .map_err(|_| {
                anyhow::anyhow!(
                    "Gitee sync: upload failed; it may have reached the server; retry to verify"
                )
            })?;
        let saved: Snippet = self.read(response, if target.is_some() { 200 } else { 201 })?;
        validate_id(&saved.id)?;
        ensure!(
            target.as_ref().is_none_or(|target| target.id == saved.id)
                && self.content(&saved, &saved.id, false)? == content,
            "Gitee sync: server did not confirm the complete private archive"
        );
        let verified: Snippet = self.get(&format!("/gists/{}", saved.id))?;
        ensure!(
            self.content(&verified, &saved.id, false)? == content,
            "Gitee sync: archive changed after upload; retry to verify and merge"
        );
        *self.target.borrow_mut() = Some(Target {
            id: saved.id.clone(),
            content,
        });
        log::info!("Gitee sync: fixed private encrypted archive saved");
        Ok(saved.id)
    }
}

#[cfg(test)]
mod live;
#[cfg(test)]
mod tests;
