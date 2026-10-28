use age::secrecy::{ExposeSecret, SecretString};
use anyhow::{Context as _, ensure};
use ely_gpui_component::{forms::TextInput, primitives::Severity};
use gpui::{App, AppContext, Context, Entity, Subscription, Window};

use super::{Gitee, Provider, Request, SyncSettings, WebDav, credentials, crypto};
use crate::{settings::SettingsStore, ssh::HostStore};

mod view;

pub struct SyncPage {
    provider: Provider,
    url: Entity<TextInput>,
    username: Entity<TextInput>,
    password: Entity<TextInput>,
    vault: Entity<TextInput>,
    token: Entity<TextInput>,
    passphrase: Entity<TextInput>,
    confirmation: Entity<TextInput>,
    include_credentials: bool,
    remember_token: bool,
    busy: bool,
    status: Option<(Severity, String)>,
    _hosts: Subscription,
}

impl SyncPage {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let saved = SettingsStore::get(cx).clone();
        let mut field = |label, placeholder, text, secret| {
            cx.new(|cx| {
                let mut input = TextInput::new(window, cx)
                    .label(label)
                    .placeholder(placeholder);
                if secret {
                    input = input.masked();
                }
                input.set_text(text, cx);
                input
            })
        };
        Self {
            provider: saved.sync.provider,
            url: field(
                "WebDAV folder URL",
                "https://example.com/dav/MioTerm/",
                saved.webdav.url,
                false,
            ),
            username: field("WebDAV username", "Username", saved.webdav.username, false),
            password: field("WebDAV password", "Not saved", String::new(), true),
            vault: field("Gitee vault", "main", saved.sync.gitee.vault, false),
            token: field(
                "Gitee token",
                "Token with gists permission",
                String::new(),
                true,
            ),
            passphrase: field(
                "Encryption passphrase",
                "At least 12 characters",
                String::new(),
                true,
            ),
            confirmation: field(
                "Confirm passphrase",
                "Required for sync",
                String::new(),
                true,
            ),
            include_credentials: saved.sync.include_credentials,
            remember_token: saved.sync.remember_token,
            busy: false,
            status: None,
            _hosts: cx.observe_global::<HostStore>(|_, cx| cx.notify()),
        }
    }

    fn settings(&self, cx: &App) -> (WebDav, SyncSettings) {
        (
            WebDav {
                url: self.url.read(cx).text().trim().into(),
                username: self.username.read(cx).text().trim().into(),
            },
            SyncSettings {
                provider: self.provider,
                gitee: Gitee {
                    vault: self.vault.read(cx).text().trim().into(),
                },
                include_credentials: self.include_credentials,
                remember_token: self.remember_token,
            },
        )
    }

    fn start(&mut self, operation: Option<bool>, cx: &mut Context<Self>) -> anyhow::Result<()> {
        if self.busy {
            return Ok(());
        }
        let (webdav, settings) = self.settings(cx);
        webdav.validate()?;
        settings.gitee.validate()?;
        let passphrase = SecretString::from(self.passphrase.read(cx).text().to_owned());
        if let Some(restore_only) = operation {
            if settings.provider == Provider::WebDav {
                webdav.endpoint()?;
            }
            crypto::validate_passphrase(&passphrase)?;
            if !restore_only {
                ensure!(
                    passphrase.expose_secret() == self.confirmation.read(cx).text(),
                    "SSH sync: encryption passphrases do not match"
                );
            }
        }
        let password = SecretString::from(if settings.provider == Provider::Gitee {
            self.token.read(cx).text().to_owned()
        } else {
            self.password.read(cx).text().to_owned()
        });
        if settings.provider == Provider::Gitee && !password.expose_secret().is_empty() {
            credentials::validate_token(password.expose_secret())?;
        }
        let forget_token = SettingsStore::get(cx).sync.remember_token && !settings.remember_token;
        SettingsStore::update(cx, |next| {
            next.webdav = webdav.clone();
            next.sync = settings.clone();
        })?;
        let key_folder = dirs::home_dir()
            .context("SSH sync: the platform names no home folder")?
            .join(".ssh/elyterminal-sync");
        let request = Request {
            settings: webdav,
            gitee: (settings.provider == Provider::Gitee).then_some(settings.gitee),
            include_credentials: settings.include_credentials,
            remember_token: settings.remember_token,
            password,
            passphrase,
            local: HostStore::get(cx).clone(),
            key_folder,
            restore_only: operation.unwrap_or(false),
        };
        for input in [
            &self.password,
            &self.token,
            &self.passphrase,
            &self.confirmation,
        ] {
            input.update(cx, |input, cx| input.set_text("", cx));
        }
        self.busy = true;
        self.status = Some((
            Severity::Info,
            match operation {
                None => "Saving synchronization settings...",
                Some(true) => "Restoring encrypted SSH archive...",
                Some(false) => "Synchronizing encrypted SSH archive...",
            }
            .into(),
        ));
        log::info!(
            "SSH sync: {:?} operation started; credentials enabled: {}",
            settings.provider,
            settings.include_credentials
        );
        let work = cx.background_executor().spawn(async move {
            if forget_token {
                credentials::save_token("", false)?;
            } else if request.gitee.is_some() && request.remember_token {
                credentials::save_token(request.password.expose_secret(), true)?;
            }
            if operation.is_none() {
                return Ok(None);
            }
            super::run(request).map(Some)
        });
        cx.spawn(async move |page, cx| {
            let result: anyhow::Result<_> = work.await;
            let message = cx.update(|cx| {
                let result = result.and_then(|restored| {
                    let Some(restored) = restored else { return Ok("Synchronization settings saved.".into()); };
                    let (hosts, keys, credentials) = (restored.hosts, restored.keys, restored.credentials);
                    HostStore::update(cx, |local| restored.apply(local))?;
                    Ok(format!("Merged {hosts} hosts, {keys} private keys and {credentials} credentials. Deletions are not propagated."))
                });
                match result {
                    Ok(message) => { log::info!("SSH sync: {message}"); (Severity::Success, message) }
                    Err(error) => { log::error!("SSH sync: operation failed: {error:#}"); (Severity::Danger, format!("{error:#}")) }
                }
            });
            let _ = page.update(cx, |page, cx| {
                page.busy = false;
                page.status = Some(message);
                cx.notify();
            });
        }).detach();
        cx.notify();
        Ok(())
    }

    fn failed(&mut self, error: anyhow::Error, cx: &mut Context<Self>) {
        log::error!("SSH sync: operation refused: {error:#}");
        self.status = Some((Severity::Danger, format!("{error:#}")));
        cx.notify();
    }
}
