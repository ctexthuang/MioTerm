use std::{io::Read, time::Duration};

use age::secrecy::{ExposeSecret, SecretString};
use anyhow::{Context as _, ensure};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use ureq::{
    Agent,
    http::{HeaderValue, StatusCode},
};
use zeroize::Zeroizing;

use super::{LIMIT, WebDav};

pub(super) struct Client {
    agent: Agent,
    url: String,
    authorization: Zeroizing<String>,
}

pub(super) struct Found {
    pub body: Vec<u8>,
    pub etag: String,
}

impl Client {
    fn authorization(&self) -> HeaderValue {
        let mut value = HeaderValue::from_str(self.authorization.as_str())
            .expect("base64 authorization has no control characters");
        value.set_sensitive(true);
        value
    }

    pub fn new(settings: &WebDav, password: &SecretString) -> anyhow::Result<Self> {
        Self::at(settings.endpoint()?.as_str(), &settings.username, password)
    }

    pub(super) fn at(url: &str, username: &str, password: &SecretString) -> anyhow::Result<Self> {
        let credentials = Zeroizing::new(format!("{username}:{}", password.expose_secret()));
        let authorization =
            Zeroizing::new(format!("Basic {}", STANDARD.encode(credentials.as_bytes())));
        let agent = Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(45)))
            .timeout_connect(Some(Duration::from_secs(10)))
            .max_redirects(0)
            .http_status_as_error(false)
            .build()
            .into();
        Ok(Self {
            agent,
            url: url.into(),
            authorization,
        })
    }

    pub fn get(&self) -> anyhow::Result<Option<Found>> {
        let mut response = self
            .agent
            .get(&self.url)
            .header("Authorization", self.authorization())
            .header("Accept-Encoding", "identity")
            .call()
            .map_err(|_| {
                anyhow::anyhow!("WebDAV: GET failed; check the server, TLS certificate and network")
            })?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        ensure!(
            response.status() == StatusCode::OK,
            "WebDAV: GET returned HTTP {}; check the URL and access credentials",
            response.status().as_u16()
        );
        let etag = response
            .headers()
            .get("etag")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_string();
        let mut body = Vec::new();
        response
            .body_mut()
            .as_reader()
            .take(LIMIT as u64 + 65537)
            .read_to_end(&mut body)
            .context("WebDAV: cannot read the encrypted archive")?;
        ensure!(
            body.len() <= LIMIT + 65536,
            "WebDAV: encrypted archive is too large"
        );
        Ok(Some(Found { body, etag }))
    }

    pub fn put(&self, body: &[u8], etag: Option<&str>) -> anyhow::Result<()> {
        let condition = match etag {
            Some(etag) => {
                ensure!(
                    etag.starts_with('"')
                        && etag.ends_with('"')
                        && etag.len() > 2
                        && etag.as_bytes()[1..etag.len() - 1]
                            .iter()
                            .all(|byte| matches!(byte, 0x21 | 0x23..=0x7e | 0x80..=0xff)),
                    "WebDAV: server provides no strong ETag; refusing an unsafe overwrite"
                );
                ("If-Match", etag)
            }
            None => ("If-None-Match", "*"),
        };
        let response = self
            .agent
            .put(&self.url)
            .header("Authorization", self.authorization())
            .header("Content-Type", "application/octet-stream")
            .header(condition.0, condition.1)
            .send(body)
            .map_err(|_| {
                anyhow::anyhow!("WebDAV: PUT failed; check the server, TLS certificate and network")
            })?;
        ensure!(
            response.status() != StatusCode::PRECONDITION_FAILED,
            "WebDAV: remote hosts changed on another device; retry to merge the latest archive"
        );
        ensure!(
            matches!(response.status().as_u16(), 200 | 201 | 204),
            "WebDAV: PUT returned HTTP {}; the folder must exist and allow writing",
            response.status().as_u16()
        );
        Ok(())
    }
}
