use ely_gpui_component::{
    buttons::{Button, ButtonVariant, SegmentedControl},
    feedback::InlineMessage,
    forms::{Input, PasswordInput, Switch, TextInput},
    primitives::IconName,
    settings::{SettingsRow, SettingsSection},
    theme::{ActiveTheme, ControlSize},
};
use gpui::{Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div};

use super::{Provider, SyncPage};
use crate::ssh::HostStore;

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
        let provider = cx.entity();
        let choose = SegmentedControl::new(
            "sync-provider",
            if self.provider == Provider::Gitee {
                "gitee"
            } else {
                "webdav"
            },
        )
        .segment("gitee", "Gitee", None)
        .segment("webdav", "WebDAV", None)
        .size(ControlSize::Sm)
        .on_change(move |key, _, cx| {
            provider.update(cx, |page, cx| {
                if page.busy {
                    return;
                }
                page.provider = if key.as_ref() == "gitee" {
                    Provider::Gitee
                } else {
                    Provider::WebDav
                };
                page.status = None;
                cx.notify();
            })
        });
        let token = cx.entity();
        let server = if self.provider == Provider::Gitee {
            SettingsSection::new("Gitee")
                .row(SettingsRow::new("Vault name").control(field(&self.vault, false)))
                .row(SettingsRow::new("Access token").control(field(&self.token, true)))
                .row(
                    SettingsRow::new("Remember Gitee token").control(
                        Switch::new("remember-sync-token", self.remember_token)
                            .disabled(self.busy)
                            .on_change(move |value, _, cx| {
                                token.update(cx, |page, cx| {
                                    page.remember_token = value;
                                    cx.notify();
                                })
                            }),
                    ),
                )
        } else {
            SettingsSection::new("WebDAV")
                .row(SettingsRow::new("Folder URL").control(field(&self.url, false)))
                .row(SettingsRow::new("Username").control(field(&self.username, false)))
                .row(SettingsRow::new("WebDAV password").control(field(&self.password, true)))
        };
        let secrets = cx.entity();
        let encryption = SettingsSection::new("Encryption")
            .row(SettingsRow::new("Encryption passphrase").control(field(&self.passphrase, true)))
            .row(SettingsRow::new("Confirm passphrase").control(field(&self.confirmation, true)))
            .row(
                SettingsRow::new("Sync SSH passwords and key passphrases").control(
                    Switch::new("sync-ssh-credentials", self.include_credentials)
                        .disabled(self.busy)
                        .on_change(move |value, _, cx| {
                            secrets.update(cx, |page, cx| {
                                page.include_credentials = value;
                                cx.notify();
                            })
                        }),
                ),
            );
        let mut actions = div().flex().flex_wrap().gap_2();
        for (id, label, icon, operation) in [
            ("sync-save", "Save", IconName::Save, None),
            ("sync-now", "Sync now", IconName::RefreshCw, Some(false)),
            ("sync-restore", "Restore", IconName::Download, Some(true)),
        ] {
            let page = cx.entity();
            actions = actions.child(
                Button::new(id, label)
                    .icon(icon)
                    .size(ControlSize::Sm)
                    .variant(if operation == Some(false) {
                        ButtonVariant::Primary
                    } else {
                        ButtonVariant::Secondary
                    })
                    .disabled(self.busy)
                    .on_click(move |_, _, cx| {
                        page.update(cx, |page, cx| {
                            if let Err(error) = page.start(operation, cx) {
                                page.failed(error, cx);
                            }
                        })
                    }),
            );
        }
        let data = HostStore::get(cx);
        let contents = SettingsSection::new("SSH archive")
            .row(SettingsRow::new("Saved hosts").description(data.hosts.len().to_string()))
            .row(
                SettingsRow::new("Hosts with explicit private keys").description(
                    data.hosts
                        .iter()
                        .filter(|host| !host.identity.is_empty())
                        .count()
                        .to_string(),
                ),
            )
            .row(SettingsRow::new("Sync").control(actions));
        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(
                SettingsSection::new("Storage").row(SettingsRow::new("Provider").control(choose)),
            )
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
