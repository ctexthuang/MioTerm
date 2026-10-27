use std::{
    collections::{HashMap, VecDeque},
    sync::mpsc::TryRecvError,
    time::{Duration, Instant},
};

use gpui::{Context, Subscription, Task};

use crate::settings::{PanelTab, Settings, SettingsStore};
use chart::{HISTORY, Scale};
use model::{Rate, Snapshot, Traffic};
use worker::{Update, Worker};

mod chart;
mod local;
mod meter;
mod model;
mod network;
mod remote;
#[cfg(test)]
mod tests;
mod view;
mod worker;

pub use worker::Target;

#[derive(Clone, Copy, PartialEq)]
enum Sort {
    Cpu,
    Memory,
}

pub struct MonitorPanel {
    target: Target,
    snapshot: Option<Snapshot>,
    traffic: Traffic,
    history: VecDeque<HashMap<String, Rate>>,
    scale: Scale,
    interface: Option<String>,
    sort: Sort,
    enabled: bool,
    paused: bool,
    waiting: bool,
    error: Option<String>,
    sampling: Option<Task<()>>,
    _settings: Subscription,
}

impl MonitorPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let enabled = visible(SettingsStore::get(cx));
        Self {
            target: Target::Local,
            snapshot: None,
            traffic: Traffic::default(),
            history: VecDeque::new(),
            scale: Scale::default(),
            interface: None,
            sort: Sort::Cpu,
            enabled,
            paused: false,
            waiting: false,
            error: None,
            sampling: None,
            _settings: cx.observe_global::<SettingsStore>(|this, cx| {
                let enabled = visible(SettingsStore::get(cx));
                if this.enabled != enabled {
                    this.enabled = enabled;
                    this.restart(cx);
                }
            }),
        }
    }

    pub fn show(&mut self, target: Target, cx: &mut Context<Self>) {
        if self.target != target {
            log::info!("monitor: target changes to {target:?}");
            self.target = target;
            self.snapshot = None;
            self.interface = None;
            self.paused = false;
            self.restart(cx);
        } else if self.sampling.is_none() && self.snapshot.is_none() && self.error.is_none() {
            self.restart(cx);
        }
    }

    fn restart(&mut self, cx: &mut Context<Self>) {
        self.sampling = None;
        self.traffic = Traffic::default();
        self.history.clear();
        self.scale = Scale::default();
        self.error = None;
        self.waiting = false;
        if !self.enabled || self.paused || self.disconnected() {
            log::debug!("monitor: sampling stopped");
            cx.notify();
            return;
        }
        log::info!("monitor: sampling starts for {:?}", self.target);
        match Worker::start(self.target.clone()) {
            Err(error) => self.error = Some(format!("{error:#}")),
            Ok(worker) => {
                self.sampling = Some(cx.spawn(async move |panel, cx| {
                    loop {
                        cx.background_executor()
                            .timer(Duration::from_millis(200))
                            .await;
                        let update = match worker.receiver.try_recv() {
                            Ok(update) => update,
                            Err(TryRecvError::Empty) => continue,
                            Err(TryRecvError::Disconnected) => {
                                Update::Failed("monitor: collector stopped unexpectedly".into())
                            }
                        };
                        let failed = matches!(update, Update::Failed(_));
                        if panel
                            .update(cx, |panel, cx| panel.accept(update, cx))
                            .is_err()
                            || failed
                        {
                            break;
                        }
                    }
                }));
            }
        }
        cx.notify();
    }

    fn disconnected(&self) -> bool {
        matches!(
            self.target,
            Target::Remote {
                disconnected: true,
                ..
            }
        )
    }

    fn accept(&mut self, update: Update, cx: &mut Context<Self>) {
        match update {
            Update::Waiting => self.waiting = true,
            Update::Failed(error) => {
                self.waiting = false;
                self.error = Some(error);
            }
            Update::Read(snapshot) => {
                if self.snapshot.is_none() || self.waiting {
                    log::info!("monitor: receiving system data from {}", snapshot.hostname);
                }
                self.waiting = false;
                let rates = self.traffic.sample(&snapshot.networks, Instant::now());
                if !rates.is_empty() {
                    self.history.push_back(rates);
                    if self.history.len() > HISTORY {
                        self.history.pop_front();
                    }
                    self.scale.update(self.network_peak());
                }
                self.snapshot = Some(snapshot);
            }
        }
        cx.notify();
    }

    fn rate(&self, sample: &HashMap<String, Rate>) -> Rate {
        sample
            .iter()
            .filter(|(name, _)| self.interface.as_ref().is_none_or(|chosen| chosen == *name))
            .fold(Rate::default(), |mut sum, (_, rate)| {
                sum.received += rate.received;
                sum.transmitted += rate.transmitted;
                sum
            })
    }

    fn network_peak(&self) -> f64 {
        self.history
            .iter()
            .map(|sample| self.rate(sample))
            .map(|rate| rate.received.max(rate.transmitted))
            .fold(0.0, f64::max)
    }

    fn select_interface(&mut self, interface: Option<String>, cx: &mut Context<Self>) {
        log::info!("monitor: network interface changed to {interface:?}");
        self.interface = interface;
        self.scale = Scale::default();
        self.scale.update(self.network_peak());
        cx.notify();
    }
}

fn visible(settings: &Settings) -> bool {
    settings.show_panel && settings.panel == PanelTab::Monitor
}
