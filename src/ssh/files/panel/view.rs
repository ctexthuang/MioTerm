use base64::{Engine as _, engine::general_purpose::STANDARD};
use ely_gpui_component::{
    buttons::{ButtonVariant, IconButton},
    feedback::Alert,
    forms::Input,
    lists::{Tree, TreeNode},
    overlays::ConfirmDialog,
    primitives::{Icon, IconName, Severity},
    theme::{ActiveTheme, ControlSize, TextSize},
    typography::{Caption, Ellipsis},
};
use gpui::{
    Context, Div, InteractiveElement, IntoElement, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder,
};

use super::RemoteFiles;
use crate::ssh::files::Entry;

pub(super) fn key(entry: &Entry) -> SharedString {
    STANDARD.encode(&entry.name.0).into()
}

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

fn tool(id: &'static str, icon: IconName, tip: &'static str) -> IconButton {
    IconButton::new(id, icon)
        .tooltip(tip)
        .variant(ButtonVariant::Ghost)
        .size(ControlSize::Sm)
}

impl RemoteFiles {
    fn pane(&self, local: bool, cx: &Context<Self>) -> Div {
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
        let (pick, open) = (cx.entity(), cx.entity());
        let rows = browser
            .listing
            .iter()
            .flat_map(|listing| &listing.entries)
            .map(|entry| {
                let row = TreeNode::new(key(entry), entry.name.shown()).icon(if entry.link {
                    IconName::Link
                } else if entry.directory {
                    IconName::Folder
                } else {
                    IconName::File
                });
                if entry.directory {
                    row
                } else {
                    row.note(size(entry.size))
                }
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
        .size_full();
        div()
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
    }

    fn confirm(&self, cx: &Context<Self>) -> Option<ConfirmDialog> {
        let transfer = self.pending.as_ref()?;
        let (close, confirm) = (cx.entity(), cx.entity());
        let direction = if transfer.upload {
            "Upload"
        } else {
            "Download"
        };
        let first = transfer.items.first()?;
        let destination = if transfer.upload {
            first.remote.parent().ok()?.shown()
        } else {
            crate::sessions::one_line(&first.local.parent()?.display().to_string())
        };
        let title = format!(
            "{direction} {} item(s) on {}?",
            transfer.items.len(),
            transfer.connection.host.label()
        );
        let body = format!(
            "{} → {destination}\nExisting files with the same names will be overwritten. Folders are merged. Symbolic links inside folders are not copied.",
            transfer.connection.host.destination()
        );
        Some(
            ConfirmDialog::new("sftp-confirm", title, body, move |_, cx| {
                close.update(cx, |panel, cx| {
                    panel.pending = None;
                    cx.notify();
                })
            })
            .confirm(direction)
            .on_confirm(move |_, cx| confirm.update(cx, |panel, cx| panel.transfer(cx))),
        )
    }
}

impl Render for RemoteFiles {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (upload, download, cancel) = (cx.entity(), cx.entity(), cx.entity());
        let busy = self.transfer.is_some();
        let blocked = busy
            || self.disconnected
            || self.local.busy
            || self.remote.busy
            || self.local.listing.is_none()
            || self.remote.listing.is_none();
        let controls = div()
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
                div().flex_1().min_w_0().children(
                    self.transfer_status
                        .as_ref()
                        .map(|text| Ellipsis::new(text.clone())),
                ),
            )
            .when(busy, |row| {
                row.child(
                    tool("sftp-cancel", IconName::X, "Cancel transfer").on_click(
                        move |_, _, cx| {
                            cancel.update(cx, |panel, cx| {
                                if let Some(cancel) = &panel.transfer {
                                    cancel.0.store(true, std::sync::atomic::Ordering::Relaxed);
                                }
                                cx.notify();
                            })
                        },
                    ),
                )
            });
        let dismiss = cx.entity();
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(self.pane(true, cx))
            .child(controls)
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
