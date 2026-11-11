use ely_gpui_component::{
    buttons::{ButtonVariant, IconButton},
    i18n::text,
    motion::ProgressBar,
    primitives::{Icon, IconName, Tooltip},
    theme::{ActiveTheme, ControlSize, TextSize},
    typography::Ellipsis,
};
use gpui::{
    Context, Div, InteractiveElement, IntoElement, ParentElement, Render,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder,
};

use super::{
    Transfers,
    model::{Job, State},
};

fn tool(
    id: impl Into<gpui::ElementId>,
    icon: IconName,
    tip: impl Into<gpui::SharedString>,
) -> IconButton {
    IconButton::new(id, icon)
        .variant(ButtonVariant::Ghost)
        .size(ControlSize::Sm)
        .tooltip(tip)
}

impl Transfers {
    fn row(&self, job: &Job, cx: &Context<Self>) -> Div {
        let colors = &cx.theme().colors;
        let item = &job.transfer.items[0];
        let local = crate::sessions::one_line(&item.local.display().to_string());
        let remote = item.remote.shown();
        let filename = item
            .remote
            .0
            .rsplit(|byte| *byte == b'/')
            .next()
            .unwrap_or_default();
        let filename = crate::sessions::one_line(&String::from_utf8_lossy(filename));
        let (source, target) = if job.transfer.upload {
            (local, remote)
        } else {
            (remote, local)
        };
        let key = match job.state {
            State::Queued => "et.transfer.queued",
            State::Running => "et.transfer.running",
            State::Cancelling => "et.transfer.cancelling",
            State::Done => "et.transfer.done",
            State::Failed => "et.transfer.failed",
            State::Cancelled => "et.transfer.cancelled",
        };
        let status = text(cx, key, &[]);
        let elapsed = job.started.map(|start| {
            job.finished
                .unwrap_or_else(std::time::Instant::now)
                .saturating_duration_since(start)
                .as_secs()
        });
        let caption =
            elapsed.map_or_else(|| status.to_string(), |secs| format!("{status} · {secs}s"));
        let color = match job.state {
            State::Failed => colors.danger,
            State::Done => colors.success,
            _ => colors.fg_muted,
        };
        let id = job.id;
        let owner = cx.entity();
        let action = if job.state.active() {
            tool(
                ("transfer-cancel", id),
                IconName::X,
                text(cx, "et.transfer.cancel", &[]),
            )
            .disabled(job.state == State::Cancelling)
            .on_click(move |_, _, cx| {
                owner.update(cx, |this, cx| {
                    this.queue.cancel(id);
                    cx.notify();
                })
            })
        } else if matches!(job.state, State::Failed | State::Cancelled) {
            tool(
                ("transfer-retry", id),
                IconName::RotateCw,
                text(cx, "et.transfer.retry", &[]),
            )
            .on_click(move |_, _, cx| owner.update(cx, |this, cx| this.retry(id, cx)))
        } else {
            let folder = item.local.parent().unwrap_or(&item.local).to_path_buf();
            tool(
                ("transfer-show", id),
                IconName::FolderOpen,
                text(cx, "et.transfer.show", &[]),
            )
            .on_click(move |_, _, cx| cx.open_with_system(&folder))
        };
        let host = job.transfer.connection.host.destination();
        let error = job.error.as_deref().map(crate::sessions::one_line);
        let detail_error = error.clone();
        let details = Tooltip::rich(move |_, _| {
            div()
                .flex()
                .flex_col()
                .gap_1()
                .whitespace_normal()
                .child(host.clone())
                .child(source.clone())
                .child(format!("→ {target}"))
                .children(detail_error.clone())
                .into_any_element()
        });
        div()
            .w_full()
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .py_1()
            .border_b_1()
            .border_color(colors.border)
            .child(
                Icon::new(if job.transfer.upload {
                    IconName::Upload
                } else {
                    IconName::Download
                })
                .color(colors.accent),
            )
            .child(
                div()
                    .id(("transfer-details", id))
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .hoverable_tooltip(details)
                    .child(Ellipsis::new(filename))
                    .child(div().text_color(color).child(Ellipsis::new(caption)))
                    .when(
                        matches!(job.state, State::Running | State::Cancelling),
                        |row| row.child(ProgressBar::indeterminate(("transfer-progress", id))),
                    )
                    .children(
                        error.map(|error| {
                            div().text_color(colors.danger).child(Ellipsis::new(error))
                        }),
                    ),
            )
            .child(action)
    }
}

impl Render for Transfers {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.active();
        let done = self
            .queue
            .jobs
            .iter()
            .filter(|job| job.state == State::Done)
            .count();
        let clear = cx.entity();
        let stop = cx.entity();
        let summary = text(
            cx,
            "et.transfer.summary",
            &[
                ("active", &active.to_string()),
                ("done", &done.to_string()),
                ("total", &self.queue.jobs.len().to_string()),
            ],
        );
        div()
            .size_full()
            .flex()
            .flex_col()
            .text_size(cx.theme().text_size(TextSize::Sm))
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap_1()
                    .px_2()
                    .py_1()
                    .border_b_1()
                    .border_color(cx.theme().colors.border)
                    .child(div().flex_1().min_w_0().child(Ellipsis::new(summary)))
                    .child(
                        tool(
                            "transfers-stop",
                            IconName::X,
                            text(cx, "et.transfer.cancel_all", &[]),
                        )
                        .disabled(active == 0)
                        .on_click(move |_, _, cx| {
                            stop.update(cx, |this, cx| {
                                let ids: Vec<_> =
                                    this.queue.jobs.iter().map(|job| job.id).collect();
                                for id in ids {
                                    this.queue.cancel(id);
                                }
                                cx.notify();
                            })
                        }),
                    )
                    .child(
                        tool(
                            "transfers-clear",
                            IconName::Trash2,
                            text(cx, "et.transfer.clear", &[]),
                        )
                        .disabled(self.queue.jobs.iter().all(|job| job.state.active()))
                        .on_click(move |_, _, cx| {
                            clear.update(cx, |this, cx| {
                                this.queue.clear();
                                cx.notify();
                            })
                        }),
                    ),
            )
            .child(
                div()
                    .id("transfer-task-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .when(self.queue.jobs.is_empty(), |list| {
                        list.child(
                            div()
                                .px_2()
                                .py_1()
                                .text_color(cx.theme().colors.fg_muted)
                                .child(text(cx, "et.transfer.empty", &[])),
                        )
                    })
                    .children(self.queue.jobs.iter().rev().map(|job| self.row(job, cx))),
            )
    }
}
