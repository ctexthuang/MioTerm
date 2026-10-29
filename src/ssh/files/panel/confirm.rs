use ely_gpui_component::{
    buttons::{Button, ButtonVariant},
    overlays::Dialog,
    theme::{ActiveTheme, TextSize},
};
use gpui::{Context, InteractiveElement, ParentElement, StatefulInteractiveElement, Styled, div};

use super::RemoteFiles;

impl RemoteFiles {
    pub(super) fn confirm(&self, cx: &Context<Self>) -> Option<Dialog> {
        let transfer = self.pending.clone()?;
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
        let body = div()
            .w_full()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_2()
            .text_size(cx.theme().text_size(TextSize::Sm))
            .text_color(cx.theme().colors.fg_muted)
            .child(transfer.connection.host.destination())
            .child(
                div()
                    .id("sftp-destination")
                    .w_full()
                    .min_w_0()
                    .overflow_x_scroll()
                    .child(destination),
            )
            .child("Existing files with the same names will be overwritten.")
            .child("Folders are merged. Symbolic links inside folders are not copied.");
        Some(
            Dialog::new("sftp-confirm", title, move |_, cx| {
                close.update(cx, |panel, cx| {
                    panel.pending = None;
                    cx.notify();
                })
            })
            .child(body)
            .action(|close| {
                Button::new("sftp-confirm-cancel", "Cancel")
                    .variant(ButtonVariant::Ghost)
                    .on_click(move |_, window, cx| close(window, cx))
            })
            .action(move |close| {
                Button::new("sftp-confirm-send", direction)
                    .primary()
                    .on_click(move |_, window, cx| {
                        close(window, cx);
                        confirm.update(cx, |panel, cx| panel.transfer(transfer.clone(), cx));
                    })
            }),
        )
    }
}
