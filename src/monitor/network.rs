use ely_gpui_component::{
    menus::{Menu, MenuItem, OverflowMenu},
    primitives::{IconName, Tooltip},
    theme::{ActiveTheme, TextSize},
    typography::{Ellipsis, tabular},
};
use gpui::{
    Context, Div, InteractiveElement, ParentElement, StatefulInteractiveElement, Styled, div,
};

use super::{
    MonitorPanel,
    chart::NetworkChart,
    model::{Rate, bytes, label},
    view::section,
};

impl MonitorPanel {
    pub(super) fn network(&self, cx: &Context<Self>) -> Div {
        let all = cx.entity();
        let mut menu = Menu::new().item(MenuItem::new("All interfaces").on_click(move |_, cx| {
            all.update(cx, |panel, cx| {
                panel.select_interface(None, cx);
            })
        }));
        if let Some(snapshot) = &self.snapshot {
            for network in &snapshot.networks {
                let owner = cx.entity();
                let name = network.name.clone();
                menu = menu.item(MenuItem::new(label(&name)).on_click(move |_, cx| {
                    owner.update(cx, |panel, cx| {
                        panel.select_interface(Some(name.clone()), cx);
                    })
                }));
            }
        }
        let mut body = div()
            .flex()
            .flex_col()
            .flex_none()
            .min_w_0()
            .gap_2()
            .child(section("Network", cx))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_1()
                    .child(
                        div().flex_1().min_w_0().child(Ellipsis::new(
                            self.interface
                                .as_deref()
                                .map(label)
                                .unwrap_or_else(|| "All interfaces".into()),
                        )),
                    )
                    .child(
                        OverflowMenu::new("monitor-interfaces", menu)
                            .icon(IconName::Filter)
                            .tooltip("Choose network interface"),
                    ),
            );
        let rate = self.history.back().map(|sample| self.rate(sample));
        let reading = |name, up, color| {
            let value = rate
                .as_ref()
                .map(|rate: &Rate| {
                    format!(
                        "{}/s",
                        bytes(if up { rate.transmitted } else { rate.received })
                    )
                })
                .unwrap_or_else(|| "--".into());
            let text = format!("{name} {value}");
            tabular(
                div()
                    .id(if up {
                        "monitor-up-rate"
                    } else {
                        "monitor-down-rate"
                    })
                    .flex_1()
                    .min_w_0(),
            )
            .text_size(cx.theme().text_size(TextSize::Sm))
            .text_color(color)
            .tooltip(Tooltip::text(text.clone()))
            .child(Ellipsis::new(text))
        };
        body = body.child(
            div()
                .w_full()
                .flex_none()
                .flex()
                .gap_2()
                .child(reading("Up", true, cx.theme().colors.chart[1]))
                .child(reading("Down", false, cx.theme().colors.chart[0])),
        );
        let samples: Vec<_> = self
            .history
            .iter()
            .map(|sample| self.rate(sample))
            .collect();
        body.child(NetworkChart::new(samples, self.scale.high))
    }
}
