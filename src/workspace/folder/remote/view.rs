use base64::{Engine as _, engine::general_purpose::STANDARD};
use ely_gpui_component::{
    buttons::{Button, ButtonVariant, IconButton},
    feedback::Alert,
    forms::Input,
    i18n::text,
    lists::{Tree, TreeNode},
    overlays::Dialog,
    primitives::{IconName, Severity},
    theme::{ActiveTheme, ControlSize},
    typography::Caption,
};
use gpui::{Context, InteractiveElement, ParentElement, StatefulInteractiveElement, Styled, div};

use crate::workspace::Workspace;

impl Workspace {
    pub(in crate::workspace) fn remote_folder_dialog(&self, cx: &Context<Self>) -> Option<Dialog> {
        let choice = self.remote_choice.as_ref()?;
        let (dismiss, pick, activate, up, home, confirm) = (
            cx.entity(),
            cx.entity(),
            cx.entity(),
            cx.entity(),
            cx.entity(),
            cx.entity(),
        );
        let parent = choice
            .listing
            .as_ref()
            .map(|listing| listing.parent.clone());
        let rows = choice
            .listing
            .iter()
            .flat_map(|listing| &listing.entries)
            .filter(|entry| entry.directory)
            .map(|entry| {
                TreeNode::new(STANDARD.encode(&entry.name.0), entry.name.shown())
                    .icon(IconName::Folder)
            });
        let selected = choice
            .selected
            .as_ref()
            .and_then(|path| path.0.rsplit(|byte| *byte == b'/').next())
            .map(|name| STANDARD.encode(name));
        let tree = Tree::new(("ssh-folder-tree", choice.generation as usize), rows)
            .selected(selected)
            .on_select(move |keys, _, cx| {
                pick.update(cx, |this, cx| {
                    if let Some(choice) = &mut this.remote_choice {
                        choice.selected = choice.listing.as_ref().and_then(|listing| {
                            let entry = listing.entries.iter().find(|entry| {
                                entry.directory
                                    && keys
                                        .iter()
                                        .any(|key| key.as_ref() == STANDARD.encode(&entry.name.0))
                            })?;
                            listing.path.join(&entry.name.0).ok()
                        });
                        cx.notify();
                    }
                })
            })
            .on_activate(move |key, _, cx| {
                activate.update(cx, |this, cx| {
                    let folder = this
                        .remote_choice
                        .as_ref()
                        .and_then(|choice| choice.listing.as_ref())
                        .and_then(|listing| {
                            let entry = listing.entries.iter().find(|entry| {
                                entry.directory && key.as_ref() == STANDARD.encode(&entry.name.0)
                            })?;
                            listing.path.join(&entry.name.0).ok()
                        });
                    if let Some(folder) = folder {
                        this.read_remote_choice(Some(folder), cx);
                    }
                })
            })
            .size_full();
        let path = div()
            .flex()
            .gap_1()
            .items_center()
            .child(
                IconButton::new("ssh-folder-up", IconName::ArrowUp)
                    .variant(ButtonVariant::Ghost)
                    .size(ControlSize::Sm)
                    .tooltip(text(cx, "et.files.parent", &[]))
                    .disabled(choice.busy || parent.is_none())
                    .on_click(move |_, _, cx| {
                        up.update(cx, |this, cx| this.read_remote_choice(parent.clone(), cx))
                    }),
            )
            .child(
                IconButton::new("ssh-folder-home", IconName::House)
                    .variant(ButtonVariant::Ghost)
                    .size(ControlSize::Sm)
                    .tooltip(text(cx, "et.files.home", &[]))
                    .on_click(move |_, _, cx| {
                        home.update(cx, |this, cx| this.read_remote_choice(None, cx))
                    }),
            )
            .child(div().flex_1().min_w_0().child(Input::new(&choice.path)));
        let body = div()
            .w(cx.theme().prose_width())
            .max_w_full()
            .flex()
            .flex_col()
            .gap_2()
            .child(Caption::new(format!(
                "{} - {}",
                choice.connection.host.label(),
                choice.connection.host.destination()
            )))
            .child(path)
            .children(
                choice
                    .busy
                    .then(|| Caption::new(text(cx, "et.files.loading", &[]))),
            )
            .children(choice.error.as_ref().map(|error| {
                div()
                    .id("ssh-folder-error-scroll")
                    .max_h(cx.theme().label_width())
                    .overflow_y_scroll()
                    .child(
                        Alert::new(
                            "ssh-folder-error",
                            Severity::Danger,
                            text(cx, "et.files.folder_failed", &[]),
                        )
                        .body(error.clone()),
                    )
            }))
            .child(
                div()
                    .id("ssh-folder-list")
                    .h_64()
                    .overflow_y_scroll()
                    .child(tree),
            );
        let blocked = choice.busy || choice.error.is_some() || choice.listing.is_none();
        let cancel = text(cx, "et.files.cancel", &[]);
        let open = text(cx, "et.files.open_remote", &[]);
        Some(
            Dialog::new(
                "ssh-folder-picker",
                text(cx, "et.files.choose_remote_folder", &[]),
                move |_, cx| dismiss.update(cx, |this, cx| this.close_remote_choice(cx)),
            )
            .child(body)
            .action(move |close| {
                Button::new("ssh-folder-cancel", cancel.clone())
                    .variant(ButtonVariant::Ghost)
                    .on_click(move |_, window, cx| close(window, cx))
            })
            .action(move |_| {
                Button::new("ssh-folder-open", open.clone())
                    .primary()
                    .disabled(blocked)
                    .on_click(move |_, window, cx| {
                        confirm.update(cx, |this, cx| this.open_remote_choice(window, cx))
                    })
            }),
        )
    }
}
