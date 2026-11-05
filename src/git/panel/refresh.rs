use std::time::Duration;

use gpui::{Context, Task};

use super::GitPanel;
use crate::settings::{PanelTab, SettingsStore};

#[derive(Default)]
pub(super) struct Refresh {
    reading: bool,
    requested: bool,
    writing: usize,
    generation: u64,
}

impl Refresh {
    pub fn write(&mut self) {
        self.writing += 1;
        self.invalidate();
    }

    pub fn wrote(&mut self) {
        self.writing -= 1;
    }

    pub fn invalidate(&mut self) {
        self.generation += 1;
        self.reading = false;
        self.requested = false;
    }

    fn idle(&self) -> bool {
        !self.reading && self.writing == 0
    }

    pub fn begin(&mut self) -> Option<u64> {
        if !self.idle() {
            self.requested = true;
            return None;
        }
        self.requested = false;
        self.reading = true;
        Some(self.generation)
    }

    pub fn finish(&mut self, generation: u64) -> bool {
        if generation != self.generation {
            return false;
        }
        self.reading = false;
        if self.writing > 0 {
            self.requested = true;
            return false;
        }
        true
    }

    pub fn again(&self) -> bool {
        self.requested && self.idle()
    }
}

impl GitPanel {
    pub(super) fn poll(cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |panel, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                if panel
                    .update(cx, |panel, cx| {
                        let settings = SettingsStore::get(cx);
                        if settings.show_panel
                            && settings.panel == PanelTab::Git
                            && panel.enabled
                            && panel.refresh.idle()
                        {
                            panel.read(cx);
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slow_reads_coalesce_and_stale_connections_cannot_replace_current_status() {
        let mut refresh = Refresh::default();
        let old = refresh.begin().unwrap();
        assert_eq!(refresh.begin(), None);
        refresh.invalidate();
        let current = refresh.begin().unwrap();
        assert!(!refresh.finish(old));
        assert!(!refresh.idle());
        assert!(refresh.finish(current));
        assert!(refresh.idle());
    }

    #[test]
    fn reads_wait_for_all_queued_mutations_and_discard_inflight_results() {
        let mut refresh = Refresh::default();
        let pending = refresh.begin().unwrap();
        refresh.write();
        refresh.write();
        assert!(!refresh.finish(pending));
        assert!(!refresh.again());
        assert_eq!(refresh.begin(), None);
        refresh.wrote();
        assert_eq!(refresh.begin(), None);
        refresh.wrote();
        assert!(refresh.again());
        assert!(refresh.begin().is_some());
        assert!(!refresh.again());
    }

    #[test]
    fn a_read_started_before_a_completed_write_stays_invalid() {
        let mut refresh = Refresh::default();
        let old = refresh.begin().unwrap();
        refresh.write();
        refresh.wrote();
        let current = refresh.begin().unwrap();
        assert!(!refresh.finish(old));
        assert!(!refresh.idle());
        assert!(refresh.finish(current));
    }
}
