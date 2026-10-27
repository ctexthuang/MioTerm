use ely_gpui_component::{
    buttons::{ButtonVariant, IconButton, SegmentedControl},
    motion::ProgressBar,
    primitives::{Icon, IconName, Tooltip},
    theme::{ActiveTheme, ControlSize, IconSize, TextSize},
    typography::{Caption, Ellipsis},
};
use gpui::{
    App, ClipboardItem, Context, Div, FontWeight, InteractiveElement, IntoElement, ParentElement,
    Render, StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder,
};

use super::{
    MonitorPanel, Sort, Target,
    meter::meter,
    model::{Usage, bytes, compact_bytes, label, uptime},
};

pub(super) fn section(title: &'static str, cx: &App) -> Div {
    div()
        .pt_3()
        .pb_1()
        .text_size(cx.theme().text_size(TextSize::Xs))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(cx.theme().colors.fg_muted)
        .child(title)
}

fn row(name: &'static str, value: String, cx: &App) -> Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_2()
        .text_size(cx.theme().text_size(TextSize::Sm))
        .child(
            div()
                .flex_none()
                .text_color(cx.theme().colors.fg_muted)
                .child(name),
        )
        .child(div().min_w_0().child(Ellipsis::new(value)))
}

fn capacity(id: &'static str, name: &'static str, usage: &Usage, cx: &App) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(meter(id, name, usage.fraction(), cx))
        .child(
            Caption::new(format!(
                "{} / {}",
                bytes(usage.used as f64),
                bytes(usage.total as f64)
            ))
            .text_color(cx.theme().colors.fg_muted),
        )
}

impl MonitorPanel {
    fn header(&self, cx: &Context<Self>) -> Div {
        let (pause, refresh) = (cx.entity(), cx.entity());
        let button = |id, icon, tip| {
            IconButton::new(id, icon)
                .tooltip(tip)
                .variant(ButtonVariant::Ghost)
                .size(ControlSize::Sm)
        };
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap_1()
            .px_2()
            .py_1()
            .child(
                Icon::new(IconName::Activity)
                    .size(IconSize::Sm)
                    .color(cx.theme().colors.fg_muted),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(cx.theme().text_size(TextSize::Sm))
                    .font_weight(FontWeight::MEDIUM)
                    .child(Ellipsis::new("System")),
            )
            .when(!self.disconnected(), |head| {
                head.child(
                    button(
                        "monitor-pause",
                        if self.paused {
                            IconName::Play
                        } else {
                            IconName::Pause
                        },
                        if self.paused {
                            "Resume monitoring"
                        } else {
                            "Pause monitoring"
                        },
                    )
                    .on_click(move |_, _, cx| {
                        pause.update(cx, |panel, cx| {
                            panel.paused = !panel.paused;
                            log::info!("monitor: paused = {}", panel.paused);
                            panel.restart(cx);
                        })
                    }),
                )
                .child(
                    button(
                        "monitor-refresh",
                        IconName::RefreshCw,
                        "Refresh system information",
                    )
                    .on_click(move |_, _, cx| {
                        refresh.update(cx, |panel, cx| {
                            panel.paused = false;
                            panel.restart(cx);
                        })
                    }),
                )
            })
    }

    fn identity(&self, cx: &Context<Self>) -> Div {
        let (name, destination, kind) = match &self.target {
            Target::Local => (
                self.snapshot
                    .as_ref()
                    .map_or("Local machine", |s| s.hostname.as_str())
                    .to_string(),
                self.snapshot
                    .as_ref()
                    .map_or("localhost", |s| s.hostname.as_str())
                    .to_string(),
                "LOCAL",
            ),
            Target::Remote { host, .. } => (host.label().to_string(), host.destination(), "SSH"),
        };
        let copy = destination.clone();
        let status = if self.disconnected() {
            "Disconnected"
        } else if self.error.is_some() {
            "Unavailable"
        } else if self.paused {
            "Paused"
        } else if self.waiting {
            "Authenticating"
        } else if self.snapshot.is_some() {
            "Live"
        } else {
            "Collecting"
        };
        div()
            .flex()
            .flex_col()
            .gap_1()
            .pb_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_none()
                            .text_size(cx.theme().text_size(TextSize::Xs))
                            .text_color(cx.theme().colors.fg_muted)
                            .child(kind),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .font_weight(FontWeight::MEDIUM)
                            .child(Ellipsis::new(label(&name))),
                    )
                    .child(Caption::new(status).text_color(if status == "Live" {
                        cx.theme().colors.success
                    } else {
                        cx.theme().colors.fg_muted
                    })),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(cx.theme().text_size(TextSize::Xs))
                            .text_color(cx.theme().colors.fg_muted)
                            .child(Ellipsis::new(destination)),
                    )
                    .child(
                        IconButton::new("monitor-copy-host", IconName::Copy)
                            .tooltip("Copy connection address")
                            .variant(ButtonVariant::Ghost)
                            .size(ControlSize::Sm)
                            .on_click(move |_, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()))
                            }),
                    ),
            )
    }

    fn metrics(&self, cx: &Context<Self>) -> Div {
        let root = div().flex().flex_col().gap_2();
        let Some(snapshot) = &self.snapshot else {
            return root;
        };
        let load = snapshot.load.map_or_else(
            || "Unavailable".into(),
            |load| format!("{:.2}  {:.2}  {:.2}", load[0], load[1], load[2]),
        );
        root.child(Caption::new(label(&snapshot.os)))
            .child(row("Uptime", uptime(snapshot.uptime), cx))
            .child(row("Load", load, cx))
            .child(meter("monitor-cpu", "CPU", snapshot.cpu / 100.0, cx))
            .child(capacity("monitor-memory", "Memory", &snapshot.memory, cx))
            .child(capacity("monitor-swap", "Swap", &snapshot.swap, cx))
            .when_some(
                snapshot
                    .disks
                    .iter()
                    .find(|disk| disk.path == "/")
                    .or_else(|| snapshot.disks.first()),
                |body, disk| body.child(capacity("monitor-disk", "Disk", &disk.usage, cx)),
            )
    }

    fn processes(&self, cx: &Context<Self>) -> Div {
        let owner = cx.entity();
        let switch = SegmentedControl::new(
            "monitor-sort",
            if self.sort == Sort::Cpu {
                "cpu"
            } else {
                "memory"
            },
        )
        .segment("cpu", "CPU", None)
        .segment("memory", "Memory", None)
        .size(ControlSize::Sm)
        .on_change(move |value, _, cx| {
            owner.update(cx, |panel, cx| {
                panel.sort = if value.as_ref() == "cpu" {
                    Sort::Cpu
                } else {
                    Sort::Memory
                };
                cx.notify();
            })
        });
        let mut processes: Vec<_> = self.snapshot.iter().flat_map(|s| &s.processes).collect();
        processes.sort_by(|a, b| match self.sort {
            Sort::Cpu => b.cpu.total_cmp(&a.cpu).then(b.memory.cmp(&a.memory)),
            Sort::Memory => b.memory.cmp(&a.memory).then(b.cpu.total_cmp(&a.cpu)),
        });
        let rows = processes.into_iter().take(6).map(|process| {
            let tip = format!("PID {} | {}", process.pid, label(&process.command));
            div()
                .id(("monitor-process", process.pid as usize))
                .flex()
                .items_center()
                .gap_2()
                .py_1()
                .text_size(cx.theme().text_size(TextSize::Xs))
                .child(
                    div()
                        .flex_none()
                        .w(cx.theme().chart().gutter)
                        .text_color(cx.theme().colors.fg_muted)
                        .overflow_hidden()
                        .child(Ellipsis::new(compact_bytes(process.memory as f64))),
                )
                .child(
                    div()
                        .flex_none()
                        .w(cx.theme().chart().gutter)
                        .overflow_hidden()
                        .child(Ellipsis::new(format!("{:.1}%", process.cpu))),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(Ellipsis::new(label(&process.command))),
                )
                .tooltip(Tooltip::text(tip))
        });
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(section("Processes", cx))
            .child(div().flex().justify_end().child(switch))
            .children(rows)
    }
}

impl Render for MonitorPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let root = div()
            .w_full()
            .flex()
            .flex_col()
            .min_h_0()
            .text_size(cx.theme().text_size(TextSize::Sm))
            .text_color(cx.theme().colors.fg)
            .child(self.header(cx));
        let mut body = div()
            .id("monitor-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px_3()
            .pb_3()
            .child(self.identity(cx));
        if self.disconnected() {
            body = body.child(Caption::new("SSH session disconnected."));
        } else if let Some(error) = &self.error {
            body = body.child(
                div()
                    .text_color(cx.theme().colors.danger)
                    .text_size(cx.theme().text_size(TextSize::Xs))
                    .child(label(error)),
            );
            if matches!(&self.target, Target::Remote { socket: None, .. }) {
                body = body.child(Caption::new(
                    "Background monitoring requires an SSH key or agent.",
                ));
            }
        } else if self.snapshot.is_some() {
            body = body
                .child(self.metrics(cx))
                .child(self.processes(cx))
                .child(self.network(cx));
        } else {
            body = body.child(Caption::new(if self.waiting {
                "Waiting for authentication in the SSH terminal."
            } else if self.paused {
                "Monitoring paused."
            } else {
                "Reading system information..."
            }));
            if !self.paused {
                body = body.child(
                    div()
                        .pt_2()
                        .child(ProgressBar::indeterminate("monitor-loading")),
                );
            }
        }
        root.h_full().child(body)
    }
}
