use std::time::Duration;

#[derive(Default)]
pub struct Recovery {
    pub attempts: usize,
    pub connected: bool,
    pub waiting: bool,
    pub error: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Connecting,
    Waiting,
    Disconnected,
    Ready,
}

pub fn disabled(enabled: &bool) -> bool {
    !enabled
}

impl Recovery {
    pub fn phase(&self, disconnected: bool) -> Phase {
        if disconnected {
            if self.waiting {
                Phase::Waiting
            } else {
                Phase::Disconnected
            }
        } else if !self.connected || self.attempts > 0 {
            Phase::Connecting
        } else {
            Phase::Ready
        }
    }

    pub fn ready(&mut self) {
        self.connected = true;
        self.attempts = 0;
        self.waiting = false;
        self.error = None;
    }

    pub fn next(&mut self, enabled: bool, code: Option<i32>) -> Option<Duration> {
        let delays = [2, 5, 10, 20, 30];
        if !enabled || !self.connected || code != Some(255) || self.waiting {
            return None;
        }
        let delay = *delays.get(self.attempts)?;
        self.attempts += 1;
        self.waiting = true;
        Some(Duration::from_secs(delay))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_established_transport_failures_retry_with_bounded_backoff() {
        let mut state = Recovery::default();
        assert!(state.next(true, Some(255)).is_none());
        state.ready();
        for code in [Some(0), Some(1), Some(130), None] {
            assert!(state.next(true, code).is_none());
        }
        assert!(state.next(false, Some(255)).is_none());
        for seconds in [2, 5, 10, 20, 30] {
            assert_eq!(
                state.next(true, Some(255)),
                Some(Duration::from_secs(seconds))
            );
            assert!(state.next(true, Some(255)).is_none());
            state.waiting = false;
        }
        assert!(state.next(true, Some(255)).is_none());
        state.ready();
        assert_eq!(state.next(true, Some(255)), Some(Duration::from_secs(2)));
    }

    #[test]
    fn preferences_default_off_and_survive_local_updates() {
        use crate::ssh::{Connections, Host};
        let json = serde_json::to_value(Host::default()).unwrap();
        assert!(json.get("auto_reconnect").is_none());
        let mut data = Connections::default();
        let saved = data
            .upsert(Host {
                auto_reconnect: true,
                ..Host::default()
            })
            .unwrap();
        data.opened(saved.clone(), 1);
        let updated = data
            .upsert(Host {
                name: "Renamed".into(),
                ..saved
            })
            .unwrap();
        let loaded: Host = serde_json::from_str(&serde_json::to_string(&updated).unwrap()).unwrap();
        assert!(loaded.auto_reconnect);
        assert!(data.recent[0].host.auto_reconnect);
    }

    #[test]
    fn connection_phases_do_not_report_old_work_after_disconnection() {
        let mut state = Recovery::default();
        assert_eq!(state.phase(false), Phase::Connecting);
        assert_eq!(state.phase(true), Phase::Disconnected);
        state.ready();
        assert_eq!(state.phase(false), Phase::Ready);
        state.next(true, Some(255)).unwrap();
        assert_eq!(state.phase(true), Phase::Waiting);
        state.waiting = false;
        assert_eq!(state.phase(false), Phase::Connecting);
        assert_eq!(state.phase(true), Phase::Disconnected);
        state.ready();
        assert_eq!(state.phase(false), Phase::Ready);
    }
}
