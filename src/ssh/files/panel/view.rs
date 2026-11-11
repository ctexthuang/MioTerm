use ely_gpui_component::{
    buttons::{ButtonVariant, IconButton},
    feedback::Alert,
    forms::Input,
    i18n::text,
    lists::{Tree, TreeNode},
    menus::ContextMenu,
    primitives::{Icon, IconName, Severity, Tooltip},
    theme::{ActiveTheme, ControlSize, TextSize},
    typography::{Caption, Ellipsis},
};
use gpui::{
    Context, Div, ExternalPaths, InteractiveElement, IntoElement, ParentElement, Render,
    SharedString, StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, rems,
};

use super::RemoteFiles;

fn size(bytes: u64) -> String {
    let mut value = bytes as f64;
    let units = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut unit = 0;
    while value >= 1024.0 && unit < units.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", units[unit])
    }
}

fn tool(id: &'static str, icon: IconName, tip: impl Into<SharedString>) -> IconButton {
    IconButton::new(id, icon)
        .tooltip(tip)
        .variant(ButtonVariant::Ghost)
        .size(ControlSize::Sm)
}

impl RemoteFiles {
    fn pane(&mut self, local: bool, cx: &Context<Self>) -> Div {
        let request = self.menu_request(local);
        let browser = self.browser(local);
        let title = if local {
            "Local".to_string()
        } else {
            self.connection
                .as_ref()
                .map_or("Remote".into(), |remote| remote.host.label().to_string())
        };
        let (up, home, refresh) = (cx.entity(), cx.entity(), cx.entity());
        let parent = browser
            .listing
            .as_ref()
            .map(|listing| listing.parent.clone());
        let header = div()
            .flex()
            .items_center()
            .gap_1()
            .px_2()
            .py_1()
            .child(
                Icon::new(if local {
                    IconName::Monitor
                } else {
                    IconName::Server
                })
                .color(cx.theme().colors.fg_muted),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(cx.theme().text_size(TextSize::Sm))
                    .child(Ellipsis::new(title)),
            )
            .child(
                tool(
                    if local { "local-up" } else { "remote-up" },
                    IconName::ArrowUp,
                    "Parent folder",
                )
                .disabled(parent.is_none())
                .on_click(move |_, _, cx| {
                    up.update(cx, |panel, cx| panel.navigate(local, parent.clone(), cx))
                }),
            )
            .child(
                tool(
                    if local { "local-home" } else { "remote-home" },
                    IconName::House,
                    "Home folder",
                )
                .on_click(move |_, _, cx| {
                    home.update(cx, |panel, cx| panel.navigate(local, None, cx))
                }),
            )
            .child(
                tool(
                    if local {
                        "local-refresh"
                    } else {
                        "remote-refresh"
                    },
                    IconName::RefreshCw,
                    "Refresh folder",
                )
                .on_click(move |_, _, cx| {
                    refresh.update(cx, |panel, cx| {
                        if local {
                            panel.refresh_local(cx)
                        } else {
                            panel.refresh_remote(cx)
                        }
                    })
                }),
            );
        let path = if local {
            &self.local_path
        } else {
            &self.remote_path
        };
        let (pick, open, context) = (cx.entity(), cx.entity(), cx.entity());
        let rows = browser
            .listing
            .iter()
            .flat_map(|listing| &listing.entries)
            .map(|entry| {
                let row =
                    TreeNode::new(self.key(local, entry), entry.name.shown()).icon(if entry.link {
                        IconName::Link
                    } else if entry.directory {
                        IconName::Folder
                    } else {
                        IconName::File
                    });
                let row = if entry.directory {
                    row
                } else {
                    row.note(size(entry.size))
                };
                let name = super::status::key(local, &entry.name.0);
                crate::files::badge(
                    row,
                    browser.marks.direct.get(name.as_ref()).copied(),
                    browser.marks.dirty.contains(name.as_ref()),
                )
            });
        let tree = Tree::new(
            (
                if local { "sftp-local" } else { "sftp-remote" },
                browser.generation,
            ),
            rows,
        )
        .multiple()
        .selected(browser.selected.clone())
        .on_select(move |keys, _, cx| {
            pick.update(cx, |panel, cx| {
                panel.browser_mut(local).selected = keys.to_vec();
                cx.notify();
            })
        })
        .on_activate(move |key, _, cx| open.update(cx, |panel, cx| panel.activate(local, key, cx)))
        .on_context(move |key, at, _, cx| {
            context.update(cx, |panel, cx| panel.context(local, key, at, cx))
        })
        .size_full();
        let tree = ContextMenu::new(
            if local {
                "sftp-local-menu"
            } else {
                "sftp-remote-menu"
            },
            self.context_menu(local, cx),
        )
        .manual(request)
        .size_full()
        .child(tree);
        div()
            .relative()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(header)
            .child(
                div()
                    .px_2()
                    .pb_1()
                    .child(Input::new(path).size(ControlSize::Sm)),
            )
            .children(browser.busy.then(|| Caption::new("Loading…").px_2()))
            .children(browser.git_error.as_ref().map(|error| {
                div()
                    .id(if local {
                        "local-git-error-scroll"
                    } else {
                        "remote-git-error-scroll"
                    })
                    .max_h(cx.theme().label_width())
                    .overflow_y_scroll()
                    .px_2()
                    .child(
                        Alert::new(
                            if local {
                                "local-git-error"
                            } else {
                                "remote-git-error"
                            },
                            Severity::Warning,
                            "Git status unavailable",
                        )
                        .body(error.clone()),
                    )
            }))
            .children(browser.error.as_ref().map(|error| {
                div()
                    .id(if local {
                        "local-files-error-scroll"
                    } else {
                        "remote-files-error-scroll"
                    })
                    .max_h(cx.theme().label_width())
                    .overflow_y_scroll()
                    .px_2()
                    .child(
                        Alert::new(
                            if local {
                                "local-files-error"
                            } else {
                                "remote-files-error"
                            },
                            Severity::Danger,
                            "Cannot read folder",
                        )
                        .body(error.clone()),
                    )
            }))
            .children(
                browser
                    .listing
                    .as_ref()
                    .filter(|listing| listing.entries.is_empty())
                    .map(|_| Caption::new("Empty folder.").px_2()),
            )
            .child(div().flex_1().min_h_0().child(tree))
            .when(!local, |pane| {
                let drop = cx.entity();
                let enabled = !self.disconnected
                    && !browser.busy
                    && browser.listing.is_some()
                    && self.pending.is_none();
                pane.drag_over::<ExternalPaths>(move |style, _, _, cx| {
                    if enabled {
                        style.bg(cx.theme().colors.selection)
                    } else {
                        style
                    }
                })
                .on_drop(move |paths: &ExternalPaths, _, cx| {
                    drop.update(cx, |panel, cx| panel.drop_upload(paths.paths(), cx))
                })
            })
            .when(cx.has_active_drag(), |pane| {
                pane.child(self.drop_surface(local, cx))
            })
    }
}

impl Render for RemoteFiles {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (upload, download) = (cx.entity(), cx.entity());
        let tasks = cx.entity();
        let active = self.transfers.read(cx).active();
        let blocked = self.pending.is_some()
            || self.disconnected
            || self.local.busy
            || self.remote.busy
            || self.local.listing.is_none()
            || self.remote.listing.is_none();
        let controls = div()
            .flex_none()
            .flex()
            .items_center()
            .gap_1()
            .px_2()
            .py_1()
            .border_y_1()
            .border_color(cx.theme().colors.border)
            .child(
                tool(
                    "sftp-upload",
                    IconName::Upload,
                    "Upload selected local files",
                )
                .disabled(blocked || self.local.selected.is_empty())
                .on_click(move |_, _, cx| upload.update(cx, |panel, cx| panel.ask(true, cx))),
            )
            .child(
                tool(
                    "sftp-download",
                    IconName::Download,
                    "Download selected remote files",
                )
                .disabled(blocked || self.remote.selected.is_empty())
                .on_click(move |_, _, cx| download.update(cx, |panel, cx| panel.ask(false, cx))),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(cx.theme().text_size(TextSize::Sm))
                    .text_color(cx.theme().colors.fg_muted)
                    .children((active > 0).then(|| {
                        Ellipsis::new(text(
                            cx,
                            "et.transfer.active",
                            &[("count", &active.to_string())],
                        ))
                    })),
            )
            .child(
                tool(
                    "sftp-tasks",
                    IconName::ArrowDownUp,
                    text(cx, "et.panel.transfers", &[]),
                )
                .when(self.show_transfers, |button| {
                    button.variant(ButtonVariant::Primary)
                })
                .on_click(move |_, _, cx| {
                    tasks.update(cx, |panel, cx| {
                        panel.show_transfers = !panel.show_transfers;
                        log::info!("sftp: inline tasks visible: {}", panel.show_transfers);
                        cx.notify();
                    });
                }),
            );
        let dismiss = cx.entity();
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(self.pane(true, cx))
            .child(controls)
            .when(self.show_transfers, |panel| {
                panel.child(
                    div()
                        .flex_1()
                        .flex_grow(0.85)
                        .min_h_0()
                        .max_h(rems(8.))
                        .overflow_hidden()
                        .border_b_1()
                        .border_color(cx.theme().colors.border)
                        .flex()
                        .flex_col()
                        .children(self.connection.as_ref().map(|connection| {
                            let mode = if connection.socket.is_some() {
                                "et.transfer.channel_shared"
                            } else {
                                "et.transfer.channel_separate"
                            };
                            let destination = connection.host.destination();
                            let endpoint = destination.clone();
                            let mode = text(cx, mode, &[]);
                            div()
                                .id("sftp-channel")
                                .flex_none()
                                .flex()
                                .items_center()
                                .gap_1()
                                .px_2()
                                .py_1()
                                .text_size(cx.theme().text_size(TextSize::Sm))
                                .text_color(cx.theme().colors.fg_muted)
                                .hoverable_tooltip(Tooltip::rich(move |_, _| {
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_1()
                                        .child(endpoint.clone())
                                        .child(mode.clone())
                                        .into_any_element()
                                }))
                                .child(
                                    Icon::new(IconName::Server).color(cx.theme().colors.fg_muted),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .child(Ellipsis::new(format!("SFTP · {destination}"))),
                                )
                        }))
                        .child(div().flex_1().min_h_0().child(self.transfers.clone())),
                )
            })
            .children(self.transfer_error.as_ref().map(|error| {
                div()
                    .id("sftp-error-scroll")
                    .max_h(cx.theme().label_width())
                    .overflow_y_scroll()
                    .px_2()
                    .child(
                        Alert::new("sftp-error", Severity::Danger, "Transfer failed")
                            .body(error.clone())
                            .on_dismiss(move |_, cx| {
                                dismiss.update(cx, |panel, cx| {
                                    panel.transfer_error = None;
                                    cx.notify();
                                })
                            }),
                    )
            }))
            .child(self.pane(false, cx))
            .children(self.confirm(cx))
    }
}
