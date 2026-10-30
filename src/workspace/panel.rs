use ely_gpui_component::{
    buttons::{ButtonVariant, IconButton},
    i18n::text,
    primitives::IconName,
    shell::ShellColumn,
    theme::ControlSize,
    typography::Caption,
};
use gpui::{
    App, Context, Div, IntoElement, ParentElement, StyleRefinement, Styled, div,
    prelude::FluentBuilder,
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
            tab_button(id, icon, text(cx, name, &[]), tab == panel)
                .on_click(move |_, _, cx| show_tab(panel, cx))
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
            .children((quiet && !remote).then(|| {
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
            .child(body(tab == PanelTab::Monitor).child(self.monitor.clone()))
            .child(body(tab != PanelTab::Monitor).child(files))
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
