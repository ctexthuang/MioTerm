use ely_gpui_component::{
    i18n::text,
    menus::{Menu, MenuItem},
    primitives::IconName,
};
use gpui::{App, Entity, EntityId};

use crate::workspace::Workspace;

/// What a right press on the row of session `id` offers.
pub fn row(workspace: &Entity<Workspace>, id: EntityId, disconnected: bool, cx: &App) -> Menu {
    let (duplicate, close) = (workspace.clone(), workspace.clone());
    let mut menu = Menu::new();
    if disconnected {
        let reconnect = workspace.clone();
        menu = menu.item(
            MenuItem::new(text(cx, "et.ssh.reconnect", &[]))
                .icon(IconName::RefreshCw)
                .on_click(move |window, cx| {
                    reconnect.update(cx, |this, cx| this.reconnect(id, false, window, cx))
                }),
        );
    }
    menu.item(
        MenuItem::new(text(cx, "et.session.duplicate", &[]))
            .icon(IconName::Copy)
            .on_click(move |window, cx| {
                duplicate.update(cx, |this, cx| this.duplicate(id, window, cx))
            }),
    )
    .separator()
    .item(
        MenuItem::new(text(cx, "et.session.close", &[]))
            .icon(IconName::X)
            .on_click(move |window, cx| {
                close.update(cx, |this, cx| {
                    let found = this
                        .sessions()
                        .iter()
                        .position(|session| session.terminal.entity_id() == id);
                    if let Some(index) = found {
                        this.close(index, window, cx);
                    }
                })
            }),
    )
}
