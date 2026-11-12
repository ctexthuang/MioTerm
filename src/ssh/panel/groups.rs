use ely_gpui_component::{
    i18n::text,
    primitives::{Disclosure, FocusRing},
    theme::{ActiveTheme, ControlSize, Radius, TextSize},
    typography::{Caption, Ellipsis},
};
use gpui::{Context, IntoElement, Role, div, prelude::*, transparent_black};

use super::HostPanel;

impl HostPanel {
    pub(super) fn group_open(&self, group: &str, searching: bool) -> bool {
        let collapsed = if searching {
            &self.search_collapsed_groups
        } else {
            &self.collapsed_groups
        };
        !collapsed.contains(group)
    }

    fn toggle_group(&mut self, group: &str, cx: &mut Context<Self>) {
        let collapsed = if self.search.read(cx).text().trim().is_empty() {
            &mut self.collapsed_groups
        } else {
            &mut self.search_collapsed_groups
        };
        let open = collapsed.remove(group);
        if !open {
            collapsed.insert(group.to_owned());
        }
        log::info!(
            "ssh: group {group:?} {}",
            if open { "expanded" } else { "collapsed" }
        );
        cx.notify();
    }

    pub(super) fn group_heading(
        &self,
        group: &str,
        count: usize,
        searching: bool,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let open = self.group_open(group, searching);
        let name = if group.is_empty() {
            text(cx, "et.ssh.ungrouped", &[])
        } else {
            group.to_owned().into()
        };
        let colors = &cx.theme().colors;
        let group = group.to_owned();
        div().pt_2().child(
            div()
                .id(format!("ssh-group-heading-{group}"))
                .role(Role::Button)
                .aria_label(format!("{name} ({count})"))
                .aria_expanded(open)
                .aria_description(text(
                    cx,
                    if open {
                        "et.ssh.collapse_group"
                    } else {
                        "et.ssh.expand_group"
                    },
                    &[],
                ))
                .tab_index(0)
                .focus_ring(cx)
                .flex()
                .items_center()
                .min_w_0()
                .h(cx.theme().control_height(ControlSize::Sm))
                .px_2()
                .gap_1p5()
                .border_1()
                .border_color(transparent_black())
                .rounded(cx.theme().radius(Radius::Sm))
                .text_size(cx.theme().text_size(TextSize::Xs))
                .text_color(colors.fg_muted)
                .cursor_pointer()
                .hover(|style| style.bg(colors.hover).text_color(colors.fg))
                .child(Disclosure::new(format!("ssh-group-arrow-{group}"), open))
                .child(div().flex_1().min_w_0().child(Ellipsis::new(name)))
                .child(div().flex_none().child(Caption::new(count.to_string())))
                .on_click(cx.listener(move |panel, _, _, cx| {
                    panel.toggle_group(&group, cx);
                })),
        )
    }
}
