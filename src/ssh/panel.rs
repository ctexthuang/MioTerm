use std::collections::BTreeMap;

use ely_gpui_component::{
    buttons::{Button, ButtonVariant, IconButton},
    feedback::Alert,
    forms::{Input, InputEvent, TextInput},
    lists::{List, ListItem},
    menus::{Menu, MenuItem, OverflowMenu},
    overlays::ConfirmDialog,
    primitives::{Icon, IconName, Severity},
    theme::{ActiveTheme, ControlSize, TextSize},
    typography::Caption,
};
use gpui::{
    AppContext, Context, Entity, EventEmitter, IntoElement, ParentElement, Render,
    StatefulInteractiveElement, Styled, Subscription, Window, div, prelude::*,
};

use super::{Host, HostStore};

mod editor;
use editor::Editor;

pub struct Connect(pub Host);

pub struct HostPanel {
    search: Entity<TextInput>,
    editor: Option<Editor>,
    deleting: Option<Host>,
    error: Option<String>,
    recent_open: bool,
    _search: Subscription,
    _store: Subscription,
    saving: Option<gpui::Task<()>>,
}

impl EventEmitter<Connect> for HostPanel {}

impl HostPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| {
            TextInput::new(window, cx)
                .label("Search hosts")
                .placeholder("Search hosts")
        });
        let changed = cx.subscribe(&search, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Changed) {
                cx.notify();
            }
        });
        let store = cx.observe_global::<HostStore>(|_, cx| cx.notify());
        Self {
            search,
            editor: None,
            deleting: None,
            error: None,
            recent_open: false,
            _search: changed,
            _store: store,
            saving: None,
        }
    }

    pub fn edit(&mut self, host: Host, quick: bool, window: &mut Window, cx: &mut Context<Self>) {
        log::info!(
            "ssh: {} editor opens for {}",
            if quick { "quick" } else { "host" },
            host.label()
        );
        self.error = None;
        self.editor = Some(Editor::new(host, quick, window, cx));
        cx.notify();
    }

    fn finish(&mut self, connect: bool, cx: &mut Context<Self>) {
        if self.saving.is_some() {
            return;
        }
        let Some(editor) = &self.editor else {
            return;
        };
        let host = match editor.host(cx) {
            Ok(host) => host,
            Err(error) => {
                self.error = Some(format!("{error:#}"));
                cx.notify();
                return;
            }
        };
        let quick = editor.quick;
        let original = editor.original.clone();
        let shared = HostStore::get(cx).hosts.iter().any(|other| {
            other.id != original.id
                && super::auth::account(other) == super::auth::account(&original)
        });
        let secret = zeroize::Zeroizing::new(editor.secret.read(cx).text().to_string());
        let save = cx.background_executor().spawn(async move {
            if host.remember_secret {
                super::auth::save(&host, &secret)?;
            }
            if original.remember_secret
                && !shared
                && (!host.remember_secret
                    || super::auth::account(&original) != super::auth::account(&host))
            {
                super::auth::forget(&original)?;
            }
            Ok::<_, anyhow::Error>(host)
        });
        self.saving = Some(cx.spawn(async move |this, cx| {
            let host = save.await;
            let _ = this.update(cx, |panel, cx| {
                panel.saving = None;
                let host = host.and_then(|host| {
                    if quick {
                        Ok(host)
                    } else {
                        HostStore::update(cx, |data| data.upsert(host))
                    }
                });
                match host {
                    Ok(host) => {
                        if !quick {
                            super::sync::service::changed(cx);
                        }
                        log::info!(
                            "ssh: {} {}",
                            if connect { "opening" } else { "saved" },
                            host.destination()
                        );
                        if connect {
                            cx.emit(Connect(host));
                        }
                        panel.editor = None;
                        panel.error = None;
                    }
                    Err(error) => {
                        log::error!("{error:#}");
                        panel.error = Some(format!("{error:#}"));
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn host_row(&self, host: &Host, recent: bool, cx: &Context<Self>) -> ListItem {
        let owner = cx.entity();
        let target = host.clone();
        let mut menu = Menu::new().item(
            MenuItem::new("Connect")
                .icon(IconName::Terminal)
                .on_click(move |_, cx| owner.update(cx, |_, cx| cx.emit(Connect(target.clone())))),
        );
        let owner = cx.entity();
        let mut target = host.clone();
        if recent {
            target.id = 0;
        }
        menu = menu.item(
            MenuItem::new(if recent { "Save as host" } else { "Edit" })
                .icon(IconName::Pencil)
                .on_click(move |window, cx| {
                    owner.update(cx, |panel, cx| {
                        panel.edit(target.clone(), false, window, cx)
                    })
                }),
        );
        if !recent {
            let owner = cx.entity();
            let target = host.clone();
            menu = menu.separator().item(
                MenuItem::new("Delete host")
                    .icon(IconName::Trash2)
                    .on_click(move |_, cx| {
                        owner.update(cx, |panel, cx| {
                            panel.deleting = Some(target.clone());
                            cx.notify();
                        })
                    }),
            );
        }
        let (owner, target) = (cx.entity(), host.clone());
        let key = if recent {
            format!("recent-host-{}-{}", host.destination(), host.identity)
        } else {
            format!("saved-host-{}", host.id)
        };
        let (connect, destination) = (cx.entity(), host.clone());
        let actions = div()
            .flex()
            .items_center()
            .gap_1()
            .child(
                IconButton::new(format!("{key}-connect"), IconName::Terminal)
                    .variant(ButtonVariant::Ghost)
                    .size(ControlSize::Sm)
                    .tooltip("Connect")
                    .on_click(move |_, _, cx| {
                        connect.update(cx, |_, cx| cx.emit(Connect(destination.clone())))
                    }),
            )
            .child(OverflowMenu::new(format!("{key}-menu"), menu).tooltip("Host actions"));
        ListItem::new(key, host.label().to_string())
            .description(host.destination())
            .leading(Icon::new(if recent {
                IconName::History
            } else {
                IconName::Server
            }))
            .on_click(move |event, _, cx| {
                if event.click_count() == 2 {
                    owner.update(cx, |_, cx| cx.emit(Connect(target.clone())));
                }
            })
            .trailing(actions)
    }

    fn delete_host(&mut self, id: u64, cx: &mut Context<Self>) {
        if self.saving.is_some() {
            return;
        }
        let Some(host) = HostStore::get(cx)
            .hosts
            .iter()
            .find(|host| host.id == id)
            .cloned()
        else {
            return;
        };
        let shared = HostStore::get(cx).hosts.iter().any(|other| {
            other.id != id && super::auth::account(other) == super::auth::account(&host)
        });
        self.deleting = None;
        let remove = cx.background_executor().spawn(async move {
            if host.remember_secret && !shared {
                super::auth::forget(&host)?;
            }
            Ok::<_, anyhow::Error>(())
        });
        self.saving = Some(cx.spawn(async move |this, cx| {
            let result = remove.await;
            let _ = this.update(cx, |panel, cx| {
                panel.saving = None;
                if let Err(error) = result.and_then(|()| {
                    HostStore::update(cx, |data| {
                        data.hosts.retain(|host| host.id != id);
                        data.recent.retain(|recent| recent.host.id != id);
                        Ok(())
                    })
                }) {
                    log::error!("ssh: cannot delete host {id}: {error:#}");
                    panel.error = Some(format!("{error:#}"));
                } else {
                    log::info!("ssh: deleted host {id} and its unshared credential");
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn delete_dialog(&self, cx: &Context<Self>) -> Option<ConfirmDialog> {
        let host = self.deleting.as_ref()?;
        let (close, remove) = (cx.entity(), cx.entity());
        let id = host.id;
        Some(
            ConfirmDialog::new(
                "ssh-delete",
                format!("Delete {}?", host.label()),
                host.destination(),
                move |_, cx| {
                    close.update(cx, |panel, cx| {
                        panel.deleting = None;
                        cx.notify();
                    })
                },
            )
            .confirm("Delete host")
            .destructive()
            .on_confirm(move |_, cx| {
                remove.update(cx, |panel, cx| {
                    panel.delete_host(id, cx);
                })
            }),
        )
    }
}

impl Render for HostPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let query = self.search.read(cx).text().trim().to_lowercase();
        let data = HostStore::get(cx);
        let mut groups: BTreeMap<&str, Vec<&Host>> = BTreeMap::new();
        for host in data.hosts.iter().filter(|host| host.matches(&query)) {
            groups
                .entry(if host.group.is_empty() {
                    "Ungrouped"
                } else {
                    &host.group
                })
                .or_default()
                .push(host);
        }
        let heading = |text: String| {
            div()
                .px_2()
                .pt_3()
                .pb_1()
                .text_size(cx.theme().text_size(TextSize::Xs))
                .text_color(cx.theme().colors.fg_muted)
                .child(text)
        };
        let mut body = div().flex().flex_col().gap_1();
        for (group, mut hosts) in groups {
            hosts.sort_by_key(|host| host.label().to_lowercase());
            let rows = hosts.into_iter().map(|host| self.host_row(host, false, cx));
            body = body
                .child(heading(group.to_string()))
                .child(List::new().children(rows));
        }
        let empty = data.hosts.is_empty();
        if empty || !data.hosts.iter().any(|host| host.matches(&query)) {
            body = body.child(
                Caption::new(if empty {
                    "No saved hosts"
                } else {
                    "No matching hosts"
                })
                .px_2()
                .py_3(),
            );
        }
        if empty {
            let owner = cx.entity();
            body = body.child(
                Button::new("ssh-first-host", "Add host")
                    .variant(ButtonVariant::Secondary)
                    .icon(IconName::Plus)
                    .on_click(move |_, window, cx| {
                        owner.update(cx, |panel, cx| {
                            panel.edit(Host::default(), false, window, cx)
                        })
                    }),
            );
        }
        let recent = data
            .recent
            .iter()
            .filter(|recent| recent.host.matches(&query));
        let count = data.recent.len();
        let toggle = cx.entity();
        body = body.child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .pt_3()
                .child(Caption::new(format!("Recent connections ({count})")).px_2())
                .child(
                    IconButton::new(
                        "toggle-recent-hosts",
                        if self.recent_open {
                            IconName::ChevronUp
                        } else {
                            IconName::ChevronDown
                        },
                    )
                    .variant(ButtonVariant::Ghost)
                    .size(ControlSize::Sm)
                    .tooltip(if self.recent_open {
                        "Hide recent connections"
                    } else {
                        "Show recent connections"
                    })
                    .on_click(move |_, _, cx| {
                        toggle.update(cx, |panel, cx| {
                            panel.recent_open = !panel.recent_open;
                            cx.notify();
                        })
                    }),
                ),
        );
        if self.recent_open {
            body = body.child(
                List::new().children(recent.map(|recent| self.host_row(&recent.host, true, cx))),
            );
        }
        let error = self
            .error
            .as_ref()
            .filter(|_| self.editor.is_none())
            .map(|error| {
                Alert::new("ssh-host-error", Severity::Danger, "Host operation failed")
                    .body(error.clone())
            });
        div()
            .size_full()
            .min_h_0()
            .flex()
            .flex_col()
            .gap_2()
            .child(Input::new(&self.search).size(ControlSize::Sm))
            .children(error)
            .child(
                div()
                    .id("ssh-host-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(body),
            )
            .children(
                self.editor
                    .as_ref()
                    .map(|editor| editor.dialog(self.error.as_ref(), self.saving.is_some(), cx)),
            )
            .children(self.delete_dialog(cx))
    }
}
