use ely_gpui_component::{
    forms::{FormField, NumberInput, Switch},
    i18n::text,
    theme::{ActiveTheme, ControlSize, TextSize},
};
use gpui::{Context, Div, ParentElement, Styled, div};

use super::{Editor, HostPanel};
use crate::ssh::Keepalive;

impl Editor {
    pub(super) fn keepalive_controls(&self, busy: bool, cx: &Context<HostPanel>) -> Div {
        let owner = cx.entity();
        let toggle = Switch::new("ssh-keepalive", self.keepalive.enabled)
            .label(text(cx, "et.ssh.keepalive", &[]))
            .disabled(busy)
            .on_change(move |enabled, _, cx| {
                owner.update(cx, |panel, cx| {
                    if let Some(editor) = &mut panel.editor {
                        editor.keepalive.enabled = enabled;
                        log::info!("ssh: keepalive draft enabled={enabled}");
                        cx.notify();
                    }
                });
            });
        let owner = cx.entity();
        let interval = NumberInput::new(
            ("ssh-keepalive-interval", self.id),
            f64::from(self.keepalive.interval),
        )
        .label(text(cx, "et.ssh.keepalive_interval", &[]))
        .range(f64::from(Keepalive::MIN), f64::from(Keepalive::MAX))
        .precision(0)
        .step(1.)
        .size(ControlSize::Sm)
        .on_change(move |interval, _, cx| {
            owner.update(cx, |panel, cx| {
                if panel.saving.is_none()
                    && let Some(editor) = &mut panel.editor
                {
                    editor.keepalive.interval = interval as u16;
                    cx.notify();
                }
            });
        });
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(toggle)
                    .child(
                        FormField::new(
                            "ssh-keepalive-interval-field",
                            text(cx, "et.ssh.keepalive_interval", &[]),
                        )
                        .child(interval),
                    ),
            )
            .child(
                div()
                    .text_size(cx.theme().text_size(TextSize::Xs))
                    .text_color(cx.theme().colors.fg_muted)
                    .child(text(cx, "et.ssh.keepalive_hint", &[])),
            )
            .child(
                Switch::new("ssh-auto-reconnect", self.auto_reconnect)
                    .label(text(cx, "et.ssh.auto_reconnect", &[]))
                    .disabled(busy)
                    .on_change({
                        let owner = cx.entity();
                        move |enabled, _, cx| {
                            owner.update(cx, |panel, cx| {
                                if let Some(editor) = &mut panel.editor {
                                    editor.auto_reconnect = enabled;
                                    log::info!("ssh: automatic reconnect draft enabled={enabled}");
                                    cx.notify();
                                }
                            });
                        }
                    }),
            )
            .child(
                div()
                    .text_size(cx.theme().text_size(TextSize::Xs))
                    .text_color(cx.theme().colors.fg_muted)
                    .child(text(cx, "et.ssh.reconnect_hint", &[])),
            )
    }
}
