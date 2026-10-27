use ely_gpui_component::{
    charts::{ChartLegend, ChartTooltip},
    theme::{ActiveTheme, TextSize},
    typography::tabular,
};
use gpui::{
    App, Bounds, ContentMask, Entity, InteractiveElement, IntoElement, ParentElement, PathBuilder,
    Pixels, Point, RenderOnce, StatefulInteractiveElement, Styled, Window, canvas, div, fill,
    point, relative, size,
};

use super::model::{Rate, bytes, compact_bytes};

pub(super) const HISTORY: usize = 60;
const SHRINK_AFTER: u8 = 10;

pub(super) struct Scale {
    pub high: f64,
    low_samples: u8,
}

impl Default for Scale {
    fn default() -> Self {
        Self {
            high: 1024.0,
            low_samples: 0,
        }
    }
}

impl Scale {
    pub fn update(&mut self, peak: f64) {
        let previous = self.high;
        let next = peak.max(1024.0).log2().ceil().exp2();
        if next > self.high {
            self.high = next;
            self.low_samples = 0;
        } else if next <= self.high / 4.0 {
            self.low_samples += 1;
            if self.low_samples == SHRINK_AFTER {
                self.high = next;
                self.low_samples = 0;
            }
        } else {
            self.low_samples = 0;
        }
        if self.high != previous {
            log::debug!(
                "monitor: network scale changes from {previous} to {}",
                self.high
            );
        }
    }
}

fn sample_x(index: usize, count: usize) -> f32 {
    (HISTORY - count + index) as f32 / (HISTORY - 1) as f32
}

fn sample_at(x: f32, count: usize) -> Option<usize> {
    let slot = (x.clamp(0.0, 1.0) * (HISTORY - 1) as f32).round() as usize;
    slot.checked_sub(HISTORY - count).filter(|ix| *ix < count)
}

#[derive(Default)]
struct State {
    bounds: Bounds<Pixels>,
    hover: Option<f32>,
    hidden: [bool; 2],
}

#[derive(IntoElement)]
pub(super) struct NetworkChart {
    samples: Vec<Rate>,
    high: f64,
}

impl NetworkChart {
    pub fn new(samples: Vec<Rate>, high: f64) -> Self {
        assert!(
            samples.len() <= HISTORY,
            "monitor: network history is too long"
        );
        Self { samples, high }
    }
}

fn stroke(
    points: impl IntoIterator<Item = Point<Pixels>>,
    width: Pixels,
    ink: gpui::Hsla,
    window: &mut Window,
) {
    let mut path = PathBuilder::stroke(width);
    let mut points = points.into_iter();
    let Some(first) = points.next() else {
        return;
    };
    path.move_to(first);
    for point in points {
        path.line_to(point);
    }
    match path.build() {
        Ok(path) => window.paint_path(path, ink),
        Err(error) => log::error!("monitor: cannot draw network curve: {error}"),
    }
}

impl RenderOnce for NetworkChart {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state: Entity<State> =
            window.use_keyed_state("monitor-network-state", cx, |_, _| State::default());
        let (hidden, hover) = {
            let state = state.read(cx);
            (state.hidden, state.hover)
        };
        let theme = cx.theme();
        let sizes = theme.chart();
        let colors = theme.colors.clone();
        let high = self.high;
        let hover = hover.and_then(|x| sample_at(x, self.samples.len()));
        let legend_state = state.clone();
        let legend = ChartLegend::new("monitor-network-legend")
            .entry(colors.chart[0], "Down", hidden[0])
            .entry(colors.chart[1], "Up", hidden[1])
            .on_toggle(move |name, _, cx| {
                legend_state.update(cx, |state, cx| {
                    let index = usize::from(name.as_ref() == "Up");
                    state.hidden[index] = !state.hidden[index];
                    log::info!("monitor: network {name} hidden={}", state.hidden[index]);
                    cx.notify();
                });
            });
        let labels = (0..=4).map(|tick| {
            let share = tick as f32 / 4.0;
            div()
                .absolute()
                .top(relative(1.0 - share))
                .w_full()
                .h_0()
                .flex()
                .items_center()
                .justify_end()
                .pr_2()
                .child(
                    tabular(div())
                        .whitespace_nowrap()
                        .text_size(theme.text_size(TextSize::Xs))
                        .text_color(colors.fg_subtle)
                        .child(format!("{}/s", compact_bytes(high * f64::from(share)))),
                )
        });
        let tooltip = hover.filter(|_| hidden != [true; 2]).map(|ix| {
            let rate = &self.samples[ix];
            let mut tip = ChartTooltip::new("");
            for (i, name, value) in [(0, "Down", rate.received), (1, "Up", rate.transmitted)] {
                if !hidden[i] {
                    tip = tip.row(Some(colors.chart[i]), name, format!("{}/s", bytes(value)));
                }
            }
            div().absolute().top_0().right_0().child(tip)
        });
        let measured = state.clone();
        let moved = state.clone();
        let (stroke_width, hairline) = (sizes.stroke.to_pixels(window.rem_size()), sizes.hairline);
        let samples = self.samples;
        let drawing = canvas(
            move |bounds, _, cx| {
                measured.update(cx, |state, _| state.bounds = bounds);
            },
            move |bounds, _, window, _| {
                let at = |x: f32, y: f32| {
                    bounds.origin + point(bounds.size.width * x, bounds.size.height * y)
                };
                window.with_content_mask(Some(ContentMask { bounds }), |window| {
                    for tick in 0..=4 {
                        window.paint_quad(fill(
                            Bounds::new(
                                at(0.0, tick as f32 / 4.0),
                                size(bounds.size.width, hairline),
                            ),
                            colors.border.opacity(0.6),
                        ));
                    }
                    if let Some(ix) = hover {
                        window.paint_quad(fill(
                            Bounds::new(
                                at(sample_x(ix, samples.len()), 0.0),
                                size(hairline, bounds.size.height),
                            ),
                            colors.fg_subtle,
                        ));
                    }
                    for (series, ink) in colors.chart.iter().take(2).enumerate() {
                        if hidden[series] {
                            continue;
                        }
                        let points = samples.iter().enumerate().map(|(ix, rate)| {
                            let value = if series == 0 {
                                rate.received
                            } else {
                                rate.transmitted
                            };
                            at(
                                sample_x(ix, samples.len()),
                                1.0 - (value / high).clamp(0.0, 1.0) as f32,
                            )
                        });
                        stroke(points, stroke_width, *ink, window);
                    }
                });
            },
        )
        .absolute()
        .inset_0();
        let plot = div()
            .id("monitor-network-plot")
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .on_mouse_move(move |event, _, cx| {
                moved.update(cx, |state, cx| {
                    let bounds = state.bounds;
                    if bounds.size.width > Pixels::ZERO {
                        let x = (event.position.x - bounds.left()) / bounds.size.width;
                        state.hover = Some(x.clamp(0.0, 1.0));
                        cx.notify();
                    }
                });
            })
            .on_hover(move |hovered, _, cx| {
                if !hovered {
                    state.update(cx, |state, cx| {
                        state.hover = None;
                        cx.notify();
                    });
                }
            })
            .child(drawing)
            .children(tooltip);
        div()
            .id("monitor-network")
            .w_full()
            .h(sizes.height * 0.6)
            .flex_none()
            .flex()
            .flex_col()
            .gap_2()
            .child(legend)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .py(sizes.inset)
                    .pr(sizes.inset)
                    .flex()
                    .child(
                        div()
                            .relative()
                            .w(sizes.gutter + sizes.inset * 2.0)
                            .flex_none()
                            .h_full()
                            .children(labels),
                    )
                    .child(plot),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_advances_one_slot_without_stretching() {
        for count in 2..HISTORY {
            let before = sample_x(count - 2, count);
            let after = sample_x(count - 2, count + 1);
            assert!((before - after - 1.0 / (HISTORY - 1) as f32).abs() < 1e-6);
            assert_eq!(sample_x(count - 1, count), 1.0);
        }
        assert_eq!(sample_x(0, HISTORY), 0.0);
        assert_eq!(sample_x(HISTORY - 1, HISTORY), 1.0);
        let shift = sample_x(5, HISTORY) - sample_x(4, HISTORY);
        assert!((shift - 1.0 / (HISTORY - 1) as f32).abs() < 1e-6);
    }

    #[test]
    fn hovering_empty_history_never_fabricates_a_sample() {
        assert_eq!(sample_at(0.0, 0), None);
        assert_eq!(sample_at(1.0, 0), None);
        assert_eq!(sample_at(0.0, 2), None);
        assert_eq!(sample_at(sample_x(0, 2), 2), Some(0));
        assert_eq!(sample_at(1.0, 2), Some(1));
    }

    #[test]
    fn scale_does_not_follow_small_rate_changes() {
        let mut scale = Scale::default();
        for peak in [0.0, 100.0, 1024.0] {
            scale.update(peak);
            assert_eq!(scale.high, 1024.0);
        }
        scale.update(15_000.0);
        assert_eq!(scale.high, 16_384.0);
        for peak in [15_001.0, 13_000.0, 12_000.0, 9_000.0] {
            scale.update(peak);
            assert_eq!(scale.high, 16_384.0);
        }
    }

    #[test]
    fn scale_grows_immediately_but_shrinks_only_after_sustained_low_traffic() {
        let mut scale = Scale::default();
        scale.update(800_000.0);
        assert_eq!(scale.high, 1_048_576.0);
        for _ in 0..SHRINK_AFTER - 1 {
            scale.update(10_000.0);
            assert_eq!(scale.high, 1_048_576.0);
        }
        scale.update(900_000.0);
        for _ in 0..SHRINK_AFTER - 1 {
            scale.update(10_000.0);
            assert_eq!(scale.high, 1_048_576.0);
        }
        scale.update(10_000.0);
        assert_eq!(scale.high, 16_384.0);
    }
}
