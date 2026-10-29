use ely_gpui_component::{
    buttons::{Button, ButtonVariant},
    forms::{Checkbox, Choice, FormField, Input, PasswordInput, Select, TextInput},
    overlays::Dialog,
};
use gpui::{App, AppContext, Context, Entity, ParentElement, Styled, Window, div};

use super::HostPanel;
use crate::ssh::{Authentication, Host};

pub(super) struct Editor {
    pub id: u64,
    pub quick: bool,
    name: Entity<TextInput>,
    address: Entity<TextInput>,
    port: Entity<TextInput>,
    user: Entity<TextInput>,
    group: Entity<TextInput>,
    identity: Entity<TextInput>,
    pub auth: Authentication,
    pub remember: bool,
    pub secret: Entity<TextInput>,
    pub original: Host,
}

impl Editor {
    pub fn new(host: Host, quick: bool, window: &mut Window, cx: &mut Context<HostPanel>) -> Self {
        let original = host.clone();
        let secret = cx.new(|cx| {
            TextInput::new(window, cx)
                .label("SSH credential")
                .placeholder(if host.remember_secret {
                    "Leave blank to keep saved credential"
                } else {
                    "Not saved"
                })
                .masked()
        });
        let mut field = |label: &'static str, placeholder: &'static str, text: String| {
            cx.new(|cx| {
                let mut input = TextInput::new(window, cx)
                    .label(label)
                    .placeholder(placeholder);
                input.set_text(text, cx);
                input
            })
        };
        let auth = if host.auth == Authentication::OpenSsh && !host.identity.is_empty() {
            Authentication::PrivateKey
        } else {
            host.auth
        };
        Self {
            id: host.id,
            quick,
            name: field("Name", "Debian LAN", host.name),
            address: field("Host", "127.0.0.1", host.address),
            port: field("Port", "22", host.port.to_string()),
            user: field("Username", "SSH default", host.user),
            group: field("Group", "Ungrouped", host.group),
            identity: field("Identity file", "Optional absolute path", host.identity),
            auth,
            remember: host.remember_secret,
            secret,
            original,
        }
    }

    pub fn host(&self, cx: &App) -> anyhow::Result<Host> {
        let text = |input: &Entity<TextInput>| input.read(cx).text().trim().to_string();
        let host = Host {
            id: self.id,
            name: text(&self.name),
            address: text(&self.address),
            port: text(&self.port)
                .parse()
                .map_err(|_| anyhow::anyhow!("ssh: port must be between 1 and 65535"))?,
            user: text(&self.user),
            group: text(&self.group),
            identity: if self.auth == Authentication::PrivateKey {
                text(&self.identity)
            } else {
                String::new()
            },
            auth: self.auth,
            remember_secret: self.remember && self.auth.has_secret(),
        };
        host.validate()?;
        Ok(host)
    }

    pub fn dialog(&self, error: Option<&String>, busy: bool, cx: &Context<HostPanel>) -> Dialog {
        let owner = cx.entity();
        let title = if self.quick {
            "Quick SSH Connection"
        } else if self.id == 0 {
            "Add SSH Host"
        } else {
            "Edit SSH Host"
        };
        let field = |id: &'static str, label: &'static str, input: &Entity<TextInput>| {
            FormField::new(id, label).child(Input::new(input))
        };
        let row = |a, b| div().flex().gap_3().child(a).child(b);
        let mut dialog = Dialog::new("ssh-editor", title, move |_, cx| {
            owner.update(cx, |panel, cx| {
                panel.editor = None;
                panel.error = None;
                cx.notify();
            })
        });
        if !self.quick {
            dialog = dialog.child(field("ssh-name", "Name", &self.name));
        }
        dialog = dialog
            .child(row(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(field("ssh-address", "Host", &self.address)),
                div().w_20().child(field("ssh-port", "Port", &self.port)),
            ))
            .child(field("ssh-user", "Username", &self.user));
        if !self.quick {
            dialog = dialog.child(field("ssh-group", "Group", &self.group));
        }
        let owner = cx.entity();
        dialog = dialog.child(
            FormField::new("ssh-authentication-field", "Authentication").child(
                Select::new(
                    "ssh-authentication",
                    Authentication::ALL
                        .into_iter()
                        .map(|mode| Choice::new(mode.id(), mode.label())),
                )
                .label("Authentication")
                .selected(self.auth.id())
                .disabled(busy)
                .on_change(move |id, _, cx| {
                    owner.update(cx, |panel, cx| {
                        if let Some(editor) = &mut panel.editor
                            && let Some(mode) = Authentication::ALL
                                .into_iter()
                                .find(|mode| mode.id() == id.as_ref())
                        {
                            editor.auth = mode;
                            editor.remember = mode == Authentication::Password;
                            cx.notify();
                        }
                    })
                }),
            ),
        );
        if self.auth == Authentication::PrivateKey {
            dialog = dialog.child(field("ssh-identity", "Private key file", &self.identity));
        }
        if self.auth.has_secret() {
            let owner = cx.entity();
            dialog = dialog.child(
                Checkbox::new("ssh-remember", self.remember)
                    .label("Save credential in application vault")
                    .disabled(busy)
                    .on_change(move |remember, _, cx| {
                        owner.update(cx, |panel, cx| {
                            if let Some(editor) = &mut panel.editor {
                                editor.remember = remember;
                                cx.notify();
                            }
                        })
                    }),
            );
            if self.remember {
                dialog = dialog.child(
                    FormField::new(
                        "ssh-secret",
                        if self.auth == Authentication::Password {
                            "Password"
                        } else {
                            "Private key passphrase (optional)"
                        },
                    )
                    .child(PasswordInput::new(&self.secret)),
                );
            }
        }
        if let Some(error) = error {
            dialog = dialog.child(
                ely_gpui_component::feedback::Alert::new(
                    "ssh-editor-error",
                    ely_gpui_component::primitives::Severity::Danger,
                    "Cannot continue",
                )
                .body(error.clone()),
            );
        }
        dialog = dialog.action(move |close| {
            Button::new("ssh-cancel", "Cancel")
                .variant(ButtonVariant::Ghost)
                .disabled(busy)
                .on_click(move |_, window, cx| close(window, cx))
        });
        if !self.quick {
            let owner = cx.entity();
            dialog = dialog.action(move |_| {
                Button::new("ssh-save", "Save")
                    .disabled(busy)
                    .on_click(move |_, _, cx| {
                        owner.update(cx, |panel, cx| panel.finish(false, cx));
                    })
            });
        }
        let owner = cx.entity();
        let label = if self.quick {
            "Connect"
        } else {
            "Save & Connect"
        };
        dialog.action(move |_| {
            Button::new("ssh-connect", label)
                .variant(ButtonVariant::Primary)
                .disabled(busy)
                .on_click(move |_, _, cx| {
                    owner.update(cx, |panel, cx| panel.finish(true, cx));
                })
        })
    }
}
