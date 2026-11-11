use ely_gpui_component::{lists::TreeDrag, theme::ActiveTheme};
use gpui::{Context, Div, InteractiveElement, Styled, div};

use super::RemoteFiles;
use crate::ssh::files::Transfer;

impl RemoteFiles {
    fn dragged_plan(&self, target_local: bool, drag: &TreeDrag) -> anyhow::Result<Transfer> {
        let source_local = !target_local;
        let keys = super::plan::selected(&self.browser(source_local).selected, drag.key());
        anyhow::ensure!(
            self.browser(source_local)
                .listing
                .as_ref()
                .is_some_and(|listing| {
                    listing
                        .entries
                        .iter()
                        .any(|entry| self.key(source_local, entry) == *drag.key())
                }),
            "sftp: drag must come from the opposite pane of this SSH session"
        );
        self.plan_selected(source_local, &keys)
    }

    pub(super) fn drop_surface(&self, local: bool, cx: &Context<Self>) -> Div {
        let (check, drop) = (cx.entity(), cx.entity());
        div()
            .absolute()
            .inset_0()
            .drag_over::<TreeDrag>(move |style, drag, _, cx| {
                if check.read(cx).pending.is_none()
                    && check.read(cx).dragged_plan(local, drag).is_ok()
                {
                    style.bg(cx.theme().colors.selection)
                } else {
                    style
                }
            })
            .on_drop(move |drag: &TreeDrag, _, cx| {
                drop.update(cx, |panel, cx| match panel.dragged_plan(local, drag) {
                    Ok(transfer) => {
                        log::info!(
                            "sftp: internal drop confirms {} items, upload={}",
                            transfer.items.len(),
                            transfer.upload
                        );
                        panel.confirm_plan(transfer, cx);
                    }
                    Err(error) => {
                        log::warn!("sftp: internal drop refused: {error:#}");
                        panel.transfer_error = Some(format!("{error:#}"));
                        cx.notify();
                    }
                });
            })
    }
}
