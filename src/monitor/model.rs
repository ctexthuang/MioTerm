use std::{collections::HashMap, time::Instant};

use anyhow::ensure;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Usage {
    pub used: u64,
    pub total: u64,
}

impl Usage {
    pub fn fraction(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            (self.used as f64 / self.total as f64).clamp(0.0, 1.0) as f32
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Process {
    pub pid: u32,
    pub command: String,
    pub memory: u64,
    pub cpu: f32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Network {
    pub name: String,
    pub received: u64,
    pub transmitted: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Disk {
    pub path: String,
    pub usage: Usage,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Snapshot {
    pub hostname: String,
    pub os: String,
    pub uptime: u64,
    pub load: Option<[f64; 3]>,
    pub cpu: f32,
    pub memory: Usage,
    pub swap: Usage,
    pub processes: Vec<Process>,
    pub networks: Vec<Network>,
    pub disks: Vec<Disk>,
}

impl Snapshot {
    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            self.cpu.is_finite() && (0.0..=100.0).contains(&self.cpu),
            "monitor: invalid CPU usage"
        );
        ensure!(
            self.load
                .is_none_or(|load| load.iter().all(|v| v.is_finite() && *v >= 0.0)),
            "monitor: invalid load average"
        );
        ensure!(
            self.memory.total > 0,
            "monitor: memory information is unavailable"
        );
        ensure!(
            self.processes.len() <= 256 && self.networks.len() <= 256 && self.disks.len() <= 256,
            "monitor: too many records in a sample"
        );
        for usage in [&self.memory, &self.swap]
            .into_iter()
            .chain(self.disks.iter().map(|d| &d.usage))
        {
            ensure!(
                usage.used <= usage.total,
                "monitor: used capacity exceeds total capacity"
            );
        }
        let strings = [&self.hostname, &self.os]
            .into_iter()
            .chain(self.processes.iter().map(|p| &p.command))
            .chain(self.networks.iter().map(|n| &n.name))
            .chain(self.disks.iter().map(|d| &d.path));
        ensure!(
            strings.into_iter().all(|s| s.len() <= 4096),
            "monitor: sample label is too long"
        );
        ensure!(
            self.processes
                .iter()
                .all(|p| p.cpu.is_finite() && (0.0..=1_000_000.0).contains(&p.cpu)),
            "monitor: invalid process CPU usage"
        );
        let mut names = std::collections::HashSet::new();
        ensure!(
            self.networks.iter().all(|n| names.insert(&n.name)),
            "monitor: duplicate network interface"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Default)]
pub struct Rate {
    pub received: f64,
    pub transmitted: f64,
}

#[derive(Default)]
pub struct Traffic {
    previous: Option<Counters>,
}

struct Counters {
    at: Instant,
    interfaces: HashMap<String, (u64, u64)>,
}

impl Traffic {
    pub fn sample(&mut self, networks: &[Network], now: Instant) -> HashMap<String, Rate> {
        let current: HashMap<_, _> = networks
            .iter()
            .map(|n| (n.name.clone(), (n.received, n.transmitted)))
            .collect();
        let mut rates = HashMap::new();
        if let Some(previous) = &self.previous {
            let seconds = now.duration_since(previous.at).as_secs_f64();
            if seconds > 0.0 {
                for (name, (received, transmitted)) in &current {
                    if let Some((rx, tx)) = previous.interfaces.get(name) {
                        rates.insert(
                            name.clone(),
                            Rate {
                                received: received.saturating_sub(*rx) as f64 / seconds,
                                transmitted: transmitted.saturating_sub(*tx) as f64 / seconds,
                            },
                        );
                    }
                }
            }
        }
        self.previous = Some(Counters {
            at: now,
            interfaces: current,
        });
        rates
    }
}

fn scaled(value: f64) -> (f64, usize) {
    let mut value = value.max(0.0);
    let mut unit = 0;
    while value >= 1024.0 && unit < 6 {
        value /= 1024.0;
        unit += 1;
    }
    (value, unit)
}

pub fn bytes(value: f64) -> String {
    let units = ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"];
    let (value, unit) = scaled(value);
    if unit == 0 {
        format!("{value:.0} {}", units[unit])
    } else {
        format!("{value:.1} {}", units[unit])
    }
}

pub fn compact_bytes(value: f64) -> String {
    let units = ["B", "K", "M", "G", "T", "P", "E"];
    let (value, unit) = scaled(value);
    if unit == 0 || value >= 100.0 {
        format!("{value:.0}{}", units[unit])
    } else {
        format!("{value:.1}{}", units[unit])
    }
}

pub fn uptime(seconds: u64) -> String {
    let hours = seconds / 3600;
    if hours >= 24 {
        format!("{}d {}h", hours / 24, hours % 24)
    } else if hours > 0 {
        format!("{hours}h {}m", seconds / 60 % 60)
    } else {
        format!("{}m", seconds / 60)
    }
}

pub fn label(text: &str) -> String {
    crate::sessions::one_line(&text.chars().take(256).collect::<String>())
}
