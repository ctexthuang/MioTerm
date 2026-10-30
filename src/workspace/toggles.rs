use ely_gpui_component::{
    buttons::{ButtonVariant, IconButton},
    i18n::text,
    primitives::IconName,
    theme::ControlSize,
};
use gpui::{App, Context, Entity};

use super::Workspace;
use crate::settings::{Settings, SettingsStore};

impl Workspace {
    /// Shows or hides a side column, as the settings keep it.
    pub(super) fn toggle(&mut self, flip: fn(&mut Settings), cx: &mut Context<Self>) {
        match SettingsStore::update(cx, flip) {
            Ok(()) => {
                let settings = SettingsStore::get(cx);
                let shown = |on: bool| if on { "shown" } else { "hidden" };
                log::info!(
                    "app: sessions {}, right column {}",
                    shown(settings.show_sessions),
                    shown(settings.show_panel)
                );
            }
            Err(error) => log::error!("app: the columns stay: {error:#}"),
        }
    }
}

/// A header button that hides or shows a column, saved.
pub(super) fn column_toggle(
    workspace: &Entity<Workspace>,
    (id, icon): (&'static str, IconName),
    (shown, [hide, show]): (bool, [&str; 2]),
    flip: fn(&mut Settings),
    cx: &App,
) -> IconButton {
    let workspace = workspace.clone();
    IconButton::new(id, icon)
        .variant(ButtonVariant::Ghost)
        .size(ControlSize::Sm)
        .tooltip(text(cx, if shown { hide } else { show }, &[]))
        .on_click(move |_, _, cx| workspace.update(cx, |this, cx| this.toggle(flip, cx)))
}
