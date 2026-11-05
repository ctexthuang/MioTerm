use anyhow::ensure;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Keepalive {
    pub enabled: bool,
    pub interval: u16,
}

impl Default for Keepalive {
    fn default() -> Self {
        Self {
            enabled: true,
            interval: 30,
        }
    }
}

impl Keepalive {
    pub const MIN: u16 = 1;
    pub const MAX: u16 = 3600;
    pub const MISSED: u8 = 3;

    pub(super) fn is_default(&self) -> bool {
        *self == Self::default()
    }

    pub(super) fn validate(self) -> anyhow::Result<()> {
        ensure!(
            (Self::MIN..=Self::MAX).contains(&self.interval),
            "ssh: keepalive interval must be between 1 and 3600 seconds"
        );
        Ok(())
    }

    pub(super) fn arguments(self) -> [String; 4] {
        let interval = if self.enabled { self.interval } else { 0 };
        [
            "-o".into(),
            format!("ServerAliveInterval={interval}"),
            "-o".into(),
            format!("ServerAliveCountMax={}", Self::MISSED),
        ]
    }
}

#[cfg(test)]
mod tests;
