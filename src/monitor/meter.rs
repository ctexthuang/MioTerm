use ely_gpui_component::{
    theme::{ActiveTheme, Palette, TextSize},
    typography::Ellipsis,
};
use gpui::{App, Hsla, InteractiveElement, IntoElement, ParentElement, Styled, div, relative};

pub(super) fn color(share: f32, colors: &Palette) -> Hsla {
    if share >= 0.95 {
        colors.danger
    } else if share >= 0.8 {
        colors.warning
    } else {
        colors.accent
    }
}

pub(super) fn meter(
    id: &'static str,
    name: &'static str,
    share: f32,
    cx: &App,
) -> impl IntoElement {
    let theme = cx.theme();
    div()
        .id(id)
        .flex()
        .flex_col()
        .gap_1p5()
        .child(
            div()
                .flex()
                .items_baseline()
                .justify_between()
                .gap_3()
                .text_size(theme.text_size(TextSize::Sm))
                .child(div().flex_1().min_w_0().child(Ellipsis::new(name)))
                .child(
                    div()
                        .flex_none()
                        .text_color(theme.colors.fg_muted)
                        .child(format!("{:.0}%", share * 100.0)),
                ),
        )
        .child(
            div()
                .h(theme.meter_track())
                .rounded_full()
                .overflow_hidden()
                .bg(theme.colors.border)
                .child(
                    div()
                        .h_full()
                        .w(relative(share))
                        .rounded_full()
                        .bg(color(share, &theme.colors)),
                ),
        )
}
