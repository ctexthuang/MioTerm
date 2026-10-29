use age::secrecy::{ExposeSecret, SecretString};
use anyhow::ensure;
use ely_gpui_component::{forms::TextInput, primitives::Severity};
use gpui::{App, AppContext, Context, Entity, Subscription, Window};

use super::{
    Gitee, Provider, SyncSettings, WebDav, crypto,
    saved::{Profile, Secrets},
    service::{self, Operation, SyncService},
};
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
    remember_password: bool,
    remember_passphrase: bool,
    auto_sync: bool,
    status: Option<(Severity, String)>,
    _hosts: Subscription,
    _service: Subscription,
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
            password: field(
                "WebDAV password",
                "Enter or use remembered password",
                String::new(),
                true,
            ),
            vault: field("Gitee vault", "main", saved.sync.gitee.vault, false),
            token: field(
                "Gitee token",
                "Token with gists permission",
                String::new(),
                true,
            ),
            passphrase: field(
                "Encryption passphrase",
                if saved.sync.remember_passphrase {
                    "Saved; leave blank to reuse"
                } else {
                    "At least 6 characters"
                },
                String::new(),
                true,
            ),
            confirmation: field(
                "Confirm passphrase",
                "Confirm a new passphrase",
                String::new(),
                true,
            ),
            include_credentials: saved.sync.include_credentials,
            remember_token: saved.sync.remember_token,
            remember_password: saved.sync.remember_password,
            remember_passphrase: saved.sync.remember_passphrase,
            auto_sync: saved.sync.auto_sync,
            status: None,
            _hosts: cx.observe_global::<HostStore>(|_, cx| cx.notify()),
            _service: cx.observe_global::<SyncService>(|page, cx| {
                page.status = None;
                cx.notify();
            }),
        }
    }

    fn settings(&self, cx: &App) -> Profile {
        Profile {
            webdav: WebDav {
                url: self.url.read(cx).text().trim().into(),
                username: self.username.read(cx).text().trim().into(),
            },
            settings: SyncSettings {
                provider: self.provider,
                gitee: Gitee {
                    vault: self.vault.read(cx).text().trim().into(),
                },
                include_credentials: self.include_credentials,
                remember_token: self.remember_token,
                remember_password: self.remember_password,
                remember_passphrase: self.remember_passphrase,
                auto_sync: self.auto_sync,
            },
        }
    }

    fn start(&mut self, operation: Operation, cx: &mut Context<Self>) -> anyhow::Result<()> {
        let secrets = Secrets {
            password: SecretString::from(if self.provider == Provider::Gitee {
                self.token.read(cx).text().to_owned()
            } else {
                self.password.read(cx).text().to_owned()
            }),
            passphrase: SecretString::from(self.passphrase.read(cx).text().to_owned()),
        };
        if !secrets.passphrase.expose_secret().is_empty() {
            crypto::validate_passphrase(&secrets.passphrase)?;
            if operation != Operation::Restore {
                ensure!(
                    secrets.passphrase.expose_secret() == self.confirmation.read(cx).text(),
                    "SSH sync: encryption passphrases do not match"
                );
            }
        } else if operation != Operation::Save && !self.remember_passphrase {
            crypto::validate_passphrase(&secrets.passphrase)?;
        }
        service::submit(self.settings(cx), secrets, operation, cx)?;
        if operation != Operation::Save {
            for input in [
                &self.password,
                &self.token,
                &self.passphrase,
                &self.confirmation,
            ] {
                input.update(cx, |input, cx| input.set_text("", cx));
            }
        }
        self.status = None;
        cx.notify();
        Ok(())
    }

    fn failed(&mut self, error: anyhow::Error, cx: &mut Context<Self>) {
        log::error!("SSH sync: operation refused: {error:#}");
        self.status = Some((Severity::Danger, format!("{error:#}")));
        cx.notify();
    }

    fn import_native(&mut self, cx: &mut Context<Self>) {
        self.status = None;
        if let Err(error) = service::import_native(cx) {
            self.failed(error, cx);
        }
        cx.notify();
    }
}
