use age::secrecy::{ExposeSecret, SecretString};
use anyhow::{Context as _, ensure};
use ely_gpui_component::{
    buttons::{Button, ButtonVariant},
    feedback::InlineMessage,
    forms::{Input, PasswordInput, TextInput},
    primitives::{IconName, Severity},
    settings::{SettingsRow, SettingsSection},
    theme::{ActiveTheme, ControlSize},
};
use gpui::{
    App, AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Subscription,
    Window, div,
};

use super::{Request, WebDav, crypto};
use crate::{settings::SettingsStore, ssh::HostStore};

pub struct SyncPage {
    url: Entity<TextInput>,
    username: Entity<TextInput>,
    password: Entity<TextInput>,
    passphrase: Entity<TextInput>,
    confirmation: Entity<TextInput>,
    busy: bool,
    status: Option<(Severity, String)>,
    _hosts: Subscription,
}

impl SyncPage {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let saved = SettingsStore::get(cx).webdav.clone();
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
            url: field(
                "WebDAV folder URL",
                "https://example.com/dav/ELYTerminal/",
                saved.url,
                false,
            ),
            username: field("WebDAV username", "Username", saved.username, false),
            password: field("WebDAV password", "Not saved", String::new(), true),
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
            busy: false,
            status: None,
            _hosts: cx.observe_global::<HostStore>(|_, cx| cx.notify()),
        }
    }

    fn settings(&self, cx: &App) -> WebDav {
        WebDav {
            url: self.url.read(cx).text().trim().into(),
            username: self.username.read(cx).text().trim().into(),
        }
    }

    fn save(&mut self, cx: &mut Context<Self>) -> anyhow::Result<WebDav> {
        let settings = self.settings(cx);
        settings.validate()?;
        SettingsStore::update(cx, |next| next.webdav = settings.clone())?;
        log::info!("WebDAV: server settings saved; passwords are not stored");
        Ok(settings)
    }

    fn start(&mut self, restore_only: bool, cx: &mut Context<Self>) -> anyhow::Result<()> {
        if self.busy {
            return Ok(());
        }
        let settings = self.settings(cx);
        settings.endpoint()?;
        let passphrase = SecretString::from(self.passphrase.read(cx).text().to_owned());
        crypto::validate_passphrase(&passphrase)?;
        if !restore_only {
            ensure!(
                passphrase.expose_secret() == self.confirmation.read(cx).text(),
                "WebDAV: encryption passphrases do not match"
            );
        }
        let key_folder = dirs::home_dir()
            .context("WebDAV: the platform names no home folder")?
            .join(".ssh/elyterminal-sync");
        let request = Request {
            settings: self.save(cx)?,
            password: SecretString::from(self.password.read(cx).text().to_owned()),
            passphrase,
            local: HostStore::get(cx).clone(),
            key_folder,
            restore_only,
        };
        for input in [&self.password, &self.passphrase, &self.confirmation] {
            input.update(cx, |input, cx| input.set_text("", cx));
        }
        self.busy = true;
        self.status = Some((
            Severity::Info,
            if restore_only {
                "Restoring encrypted SSH archive..."
            } else {
                "Synchronizing hosts and private keys..."
            }
            .into(),
        ));
        log::info!(
            "WebDAV: {} started",
            if restore_only { "restore" } else { "sync" }
        );
        let work = cx
            .background_executor()
            .spawn(async move { super::run(request) });
        cx.spawn(async move |page, cx| {
            let result = work.await;
            let message = cx.update(|cx| {
                let result = result.and_then(|restored| {
                    let (hosts, keys) = (restored.hosts, restored.keys);
                    HostStore::update(cx, |local| restored.apply(local))?;
                    Ok(format!("Merged {hosts} hosts and {keys} private keys. Different versions are kept; deletions are not propagated."))
                });
                match result {
                    Ok(message) => {
                        log::info!("WebDAV: {message}");
                        (Severity::Success, message)
                    }
                    Err(error) => {
                        log::error!("WebDAV: operation failed: {error:#}");
                        (Severity::Danger, format!("{error:#}"))
                    }
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
        log::error!("WebDAV: operation refused: {error:#}");
        self.status = Some((Severity::Danger, format!("{error:#}")));
        cx.notify();
    }
}

impl Render for SyncPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let field = |input: &Entity<TextInput>, secret| {
            div()
                .w(cx.theme().prose_width())
                .min_w_0()
                .child(if secret {
                    PasswordInput::new(input).into_any_element()
                } else {
                    Input::new(input).into_any_element()
                })
        };
        let server = SettingsSection::new("WebDAV")
            .row(SettingsRow::new("Folder URL").control(field(&self.url, false)))
            .row(SettingsRow::new("Username").control(field(&self.username, false)))
            .row(SettingsRow::new("WebDAV password").control(field(&self.password, true)));
        let encryption = SettingsSection::new("End-to-end encryption")
            .row(SettingsRow::new("Encryption passphrase").control(field(&self.passphrase, true)))
            .row(SettingsRow::new("Confirm passphrase").control(field(&self.confirmation, true)));
        let data = HostStore::get(cx);
        let count = data
            .hosts
            .iter()
            .filter(|host| !host.identity.is_empty())
            .count();
        let save = cx.entity();
        let sync = cx.entity();
        let restore = cx.entity();
        let actions = div()
            .flex()
            .flex_wrap()
            .gap_2()
            .child(
                Button::new("webdav-save", "Save")
                    .icon(IconName::Save)
                    .size(ControlSize::Sm)
                    .disabled(self.busy)
                    .variant(ButtonVariant::Secondary)
                    .on_click(move |_, _, cx| {
                        save.update(cx, |page, cx| match page.save(cx) {
                            Ok(_) => {
                                page.status = Some((
                                    Severity::Success,
                                    "Server settings saved. Passwords are not stored.".into(),
                                ));
                                cx.notify();
                            }
                            Err(error) => page.failed(error, cx),
                        })
                    }),
            )
            .child(
                Button::new("webdav-sync", "Sync now")
                    .icon(IconName::RefreshCw)
                    .primary()
                    .size(ControlSize::Sm)
                    .loading(self.busy)
                    .on_click(move |_, _, cx| {
                        sync.update(cx, |page, cx| {
                            if let Err(error) = page.start(false, cx) {
                                page.failed(error, cx);
                            }
                        })
                    }),
            )
            .child(
                Button::new("webdav-restore", "Restore")
                    .icon(IconName::Download)
                    .size(ControlSize::Sm)
                    .disabled(self.busy)
                    .variant(ButtonVariant::Secondary)
                    .on_click(move |_, _, cx| {
                        restore.update(cx, |page, cx| {
                            if let Err(error) = page.start(true, cx) {
                                page.failed(error, cx);
                            }
                        })
                    }),
            );
        let contents = SettingsSection::new("SSH archive")
            .row(SettingsRow::new("Saved hosts").description(data.hosts.len().to_string()))
            .row(
                SettingsRow::new("Hosts with explicit private keys").description(count.to_string()),
            )
            .row(SettingsRow::new("Sync").control(actions));
        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(server)
            .child(encryption)
            .child(contents)
            .children(
                self.status
                    .as_ref()
                    .map(|(severity, text)| InlineMessage::new(*severity, text.clone())),
            )
    }
}
