use std::{
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

const FILE: &str = "mioterm-ssh-v1.age.base64";
const SNAPSHOT_LIMIT: usize = 512 * 1024;
const RESPONSE_LIMIT: usize = 2 * 1024 * 1024;

pub(super) struct Client {
    agent: Agent,
    base: String,
    authorization: Zeroizing<String>,
    description: String,
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
        "Gitee sync: invalid snapshot identifier"
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
        })
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

    pub fn pull(&self, passphrase: &SecretString) -> anyhow::Result<Option<Archive>> {
        let mut snippets = BTreeMap::new();
        let mut complete = false;
        for page in 1..=10 {
            let batch: Vec<Summary> = self.get(&format!("/gists?page={page}&per_page=100"))?;
            let count = batch.len();
            for snippet in batch {
                if snippet.description.as_deref() == Some(self.description.as_str()) {
                    validate_id(&snippet.id)?;
                    ensure!(
                        !snippet.public,
                        "Gitee sync: a vault snapshot is public; make it private before syncing"
                    );
                    snippets.insert(snippet.id.clone(), snippet);
                }
            }
            if count < 100 {
                complete = true;
                break;
            }
        }
        ensure!(
            complete,
            "Gitee sync: more than 1000 snippets; refusing an incomplete synchronization"
        );
        if snippets.is_empty() {
            return Ok(None);
        }
        let mut ordered: Vec<_> = snippets.into_values().collect();
        ordered.sort_by(|left, right| {
            right
                .created_at
                .cmp(&left.created_at)
                .then(right.id.cmp(&left.id))
        });
        let mut archive = Archive::default();
        let mut seen = BTreeSet::new();
        let mut reads = 0;
        for snippet in ordered {
            if archive.snapshots.contains(&snippet.id) {
                continue;
            }
            reads += 1;
            ensure!(
                reads <= 32,
                "Gitee sync: too many unmerged snapshots; restore and consolidate older vaults first"
            );
            let detail: Snippet = self.get(&format!("/gists/{}", snippet.id))?;
            ensure!(
                detail.id == snippet.id
                    && !detail.public
                    && detail.description.as_deref() == Some(self.description.as_str()),
                "Gitee sync: snapshot identity or privacy changed"
            );
            let file = detail
                .files
                .get(FILE)
                .context("Gitee sync: snapshot archive is missing")?;
            ensure!(
                !detail.truncated && !file.truncated,
                "Gitee sync: snapshot was truncated; refusing incomplete data"
            );
            let content = file.content.as_ref().context(
                "Gitee sync: snapshot has no inline archive; refusing external download URLs",
            )?;
            ensure!(
                content.len() <= SNAPSHOT_LIMIT * 2,
                "Gitee sync: snapshot is too large"
            );
            let bytes = STANDARD
                .decode(content)
                .context("Gitee sync: invalid archive encoding")?;
            ensure!(
                bytes.len() <= SNAPSHOT_LIMIT,
                "Gitee sync: snapshot exceeds 512 KiB"
            );
            let found = crypto::decrypt(&bytes, passphrase)?;
            archive.merge(found)?;
            seen.insert(detail.id);
        }
        archive.snapshots.extend(seen);
        archive.validate()?;
        Ok(Some(archive))
    }

    pub fn put(&self, encrypted: &[u8]) -> anyhow::Result<String> {
        ensure!(
            encrypted.len() <= SNAPSHOT_LIMIT,
            "Gitee sync: archive exceeds the 512 KiB snapshot limit; use WebDAV for larger vaults"
        );
        let payload = serde_json::json!({
            "public": false,
            "description": self.description,
            "files": { FILE: { "content": STANDARD.encode(encrypted) } }
        });
        let body = serde_json::to_vec(&payload).context("Gitee sync: cannot encode upload")?;
        let response = self
            .agent
            .post(format!("{}/gists", self.base))
            .header("Authorization", self.authorization())
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .send(body.as_slice())
            .map_err(|_| {
                anyhow::anyhow!(
                    "Gitee sync: upload failed; it may have reached the server; retry safely"
                )
            })?;
        let created: Snippet = self.read(response, 201)?;
        validate_id(&created.id)?;
        ensure!(
            !created.public && created.description.as_deref() == Some(self.description.as_str()),
            "Gitee sync: server did not confirm a private vault snapshot; inspect your Gitee snippets"
        );
        ensure!(
            !created.truncated
                && created.files.get(FILE).is_some_and(|file| {
                    !file.truncated
                        && file.content.as_deref() == Some(STANDARD.encode(encrypted).as_str())
                }),
            "Gitee sync: server did not confirm the complete encrypted snapshot; retry to verify it"
        );
        log::info!("Gitee sync: private encrypted snapshot created");
        Ok(created.id)
    }
}

#[cfg(test)]
mod live;
#[cfg(test)]
mod tests;
