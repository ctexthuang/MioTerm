use ely_gpui_component::{
    buttons::{Button, ButtonVariant, IconButton},
    i18n::text,
    layout::Collapsible,
    primitives::IconName,
    shell::ShellColumn,
    theme::ControlSize,
    typography::Caption,
};
use gpui::{
    App, Context, Div, Focusable, InteractiveElement, IntoElement, MouseButton, ParentElement,
    StyleRefinement, Styled, div, prelude::FluentBuilder,
};

use super::{Workspace, widths::Side};
use crate::settings::{PanelTab, SettingsStore};

impl Workspace {
    pub(super) fn right_panel(&self, end: Div, cx: &Context<Self>) -> ShellColumn {
        let tab = SettingsStore::get(cx).panel;
        let switch = [
            (
                PanelTab::Monitor,
                "panel-monitor",
                IconName::Activity,
                "et.panel.monitor",
            ),
            (
                PanelTab::Files,
                "panel-files",
                IconName::Folder,
                "et.panel.files",
            ),
            (
                PanelTab::Git,
                "panel-git",
                IconName::GitBranch,
                "et.panel.git",
            ),
        ]
        .into_iter()
        .map(|(panel, id, icon, name)| {
            let workspace = cx.entity();
            div()
                .on_mouse_down(MouseButton::Right, move |_, window, cx| {
                    if panel == PanelTab::Files && SettingsStore::get(cx).files_menu {
                        workspace.update(cx, |this, cx| {
                            this.flip_folder_actions(cx);
                            if let Some(session) = this.sessions.get(this.active) {
                                window.focus(&session.terminal.focus_handle(cx), cx);
                            }
                        });
                    }
                })
                .child(
                    tab_button(id, icon, text(cx, name, &[]), tab == panel)
                        .on_click(move |_, _, cx| show_tab(panel, cx)),
                )
        });
        let session = self.sessions.get(self.active);
        let remote = session.is_some_and(|session| session.remote.is_some());
        let quiet = session.is_some_and(|session| !session.reports);
        let body = |on: bool| {
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .flex_col()
                .when(!on, |body| body.hidden())
        };
        let files = div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .children((quiet && !remote && tab == PanelTab::Files).then(|| {
                Caption::new(text(cx, "et.panel.no_folder", &[]))
                    .px_3()
                    .pt_2()
            }))
            .child(
                body(tab == PanelTab::Git).p_2().child(
                    self.git
                        .clone()
                        .cached(StyleRefinement::default().size_full()),
                ),
            )
            .child(body(tab == PanelTab::Files).child(if remote {
                self.remote_files.clone().into_any_element()
            } else {
                div()
                    .size_full()
                    .p_2()
                    .child(self.files.clone())
                    .into_any_element()
            }));
        self.side_column(Side::Panel, cx)
            .action(end)
            .child(
                div()
                    .flex_none()
                    .flex()
                    .justify_center()
                    .gap_1()
                    .px_2()
                    .py_2()
                    .children(switch),
            )
            .child(div().flex_none().child(self.folder_actions(cx)))
            .child(body(tab == PanelTab::Monitor).child(self.monitor.clone()))
            .child(body(matches!(tab, PanelTab::Git | PanelTab::Files)).child(files))
    }
}

pub(super) fn tab_button(
    id: &'static str,
    icon: IconName,
    name: impl Into<gpui::SharedString>,
    selected: bool,
) -> IconButton {
    IconButton::new(id, icon)
        .variant(if selected {
            ButtonVariant::Primary
        } else {
            ButtonVariant::Ghost
        })
        .size(ControlSize::Sm)
        .tooltip(name)
}

fn show_tab(panel: PanelTab, cx: &mut App) {
    if SettingsStore::get(cx).panel == panel {
        return;
    }
    match SettingsStore::update(cx, |settings| settings.panel = panel) {
        Ok(()) => log::info!("app: the right column shows {panel:?}"),
        Err(error) => log::error!("app: the right column stays: {error:#}"),
    }
}

impl Workspace {
    /// Shows Files, then shows or hides its folder row.
    fn flip_folder_actions(&mut self, cx: &mut Context<Self>) {
        show_tab(PanelTab::Files, cx);
        if SettingsStore::get(cx).panel != PanelTab::Files {
            return;
        }
        self.folder_actions = !self.folder_actions;
        let shown = if self.folder_actions {
            "shown"
        } else {
            "hidden"
        };
        log::info!("files: folder actions {shown}");
        cx.notify();
    }

    pub(super) fn folder_actions(&self, cx: &Context<Self>) -> Collapsible {
        let settings = SettingsStore::get(cx);
        let shown = self.folder_actions && settings.files_menu && settings.panel == PanelTab::Files;
        let workspace = cx.entity();
        let remote = self
            .sessions
            .get(self.active)
            .is_some_and(|session| session.remote.is_some());
        let choose = Button::new(
            "choose-session-folder",
            text(
                cx,
                if remote {
                    "et.files.choose_remote_folder"
                } else {
                    "et.files.choose_folder"
                },
                &[],
            ),
        )
        .variant(ButtonVariant::Ghost)
        .size(ControlSize::Sm)
        .icon(if remote {
            IconName::Server
        } else {
            IconName::Folder
        })
        .full_width()
        .disabled(self.choosing_folder)
        .on_click(move |_, window, cx| {
            workspace.update(cx, |this, cx| {
                this.folder_actions = false;
                log::info!("files: folder actions hidden for chooser");
                if let Some(session) = this.sessions.get(this.active) {
                    window.focus(&session.terminal.focus_handle(cx), cx);
                }
                cx.notify();
                this.choose_folder(window, cx);
            });
        });
        Collapsible::new("files-folder-actions", shown).child(div().px_2().pt_2().child(choose))
    }
}
