use ely_gpui_component::{
    i18n::text,
    menus::{Menu, MenuItem},
    primitives::IconName,
};
use gpui::{Context, Pixels, Point, SharedString};

use super::RemoteFiles;
use crate::ssh::files::Transfer;

pub(super) struct Ask {
    number: u64,
    at: Option<Point<Pixels>>,
    transfer: Option<Transfer>,
}

impl RemoteFiles {
    pub(super) fn context(
        &mut self,
        local: bool,
        key: &SharedString,
        at: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let keys = super::plan::selected(&self.browser(local).selected, key);
        self.menu_number = self.menu_number.saturating_add(1);
        let number = self.menu_number;
        let transfer = match self.plan_selected(local, &keys) {
            Ok(transfer) => Some(transfer),
            Err(error) => {
                log::warn!("sftp: transfer menu disabled: {error:#}");
                None
            }
        };
        self.browser_mut(local).selected = keys;
        self.browser_mut(local).menu = Some(Ask {
            number,
            at: Some(at),
            transfer,
        });
        log::info!(
            "sftp: {} context menu opened",
            if local { "upload" } else { "download" }
        );
        cx.notify();
    }

    pub(super) fn menu_request(&mut self, local: bool) -> Option<(u64, Point<Pixels>)> {
        let menu = self.browser_mut(local).menu.as_mut()?;
        Some((menu.number, menu.at.take()?))
    }

    pub(super) fn context_menu(&self, local: bool, cx: &Context<Self>) -> Menu {
        let transfer = self
            .browser(local)
            .menu
            .as_ref()
            .and_then(|menu| menu.transfer.clone());
        let owner = cx.entity();
        Menu::new().item(
            MenuItem::new(text(
                cx,
                if local {
                    "et.transfer.upload"
                } else {
                    "et.transfer.download"
                },
                &[],
            ))
            .icon(if local {
                IconName::Upload
            } else {
                IconName::Download
            })
            .disabled(self.pending.is_some() || self.disconnected || transfer.is_none())
            .on_click(move |_, cx| {
                if let Some(transfer) = &transfer {
                    owner.update(cx, |panel, cx| panel.confirm_plan(transfer.clone(), cx));
                }
            }),
        )
    }
}
