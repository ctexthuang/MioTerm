use std::time::{Duration, Instant};

use anyhow::Context as _;

pub(super) struct Queue {
    enabled: bool,
    due: Option<Instant>,
}

impl Queue {
    pub fn new(enabled: bool) -> Self {
        Self { enabled, due: None }
    }

    pub fn configure(&mut self, enabled: bool) {
        self.enabled = enabled;
        self.due = None;
    }

    pub fn changed(&mut self, now: Instant) -> anyhow::Result<Option<Duration>> {
        if !self.enabled {
            return Ok(None);
        }
        let mut random = [0];
        loop {
            getrandom::fill(&mut random)
                .context("SSH sync: cannot choose synchronization delay")?;
            if random[0] < 240 {
                break;
            }
        }
        let delay = Duration::from_secs(10 + u64::from(random[0] % 20));
        self.defer(now, delay);
        Ok(Some(delay))
    }

    fn defer(&mut self, now: Instant, delay: Duration) {
        self.due = self.enabled.then_some(now + delay);
    }

    pub fn ready(&self, now: Instant) -> bool {
        self.due.is_some_and(|due| now >= due)
    }

    pub fn started(&mut self) {
        self.due = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_and_configuration_changes_never_schedule_network_operations() {
        let now = Instant::now();
        let mut queue = Queue::new(true);
        assert!(!queue.ready(now + Duration::from_secs(86400)));
        queue.defer(now, Duration::from_secs(10));
        queue.configure(true);
        assert!(!queue.ready(now + Duration::from_secs(86400)));
        queue.configure(false);
        assert!(queue.changed(now).unwrap().is_none());
        queue.configure(true);
        assert!(!queue.ready(now + Duration::from_secs(86400)));
    }

    #[test]
    fn every_change_replaces_the_previous_deadline_with_ten_to_twenty_nine_seconds() {
        let now = Instant::now();
        let mut queue = Queue::new(true);
        for second in 0..100 {
            let edited = now + Duration::from_secs(second);
            let delay = queue.changed(edited).unwrap().unwrap();
            assert!((10..30).contains(&delay.as_secs()));
            assert!(!queue.ready(edited + delay - Duration::from_millis(1)));
            assert!(queue.ready(edited + delay));
        }
    }

    #[test]
    fn rapid_edits_wait_from_the_last_change() {
        let mut queue = Queue::new(true);
        let now = Instant::now();
        queue.defer(now, Duration::from_secs(10));
        queue.defer(now + Duration::from_secs(9), Duration::from_secs(29));
        assert!(!queue.ready(now + Duration::from_secs(37)));
        assert!(queue.ready(now + Duration::from_secs(38)));
    }

    #[test]
    fn started_requests_never_retry_unless_another_edit_arrives() {
        let now = Instant::now();
        let mut queue = Queue::new(true);
        queue.changed(now).unwrap();
        queue.started();
        assert!(!queue.ready(now + Duration::from_secs(86400)));
        queue.changed(now).unwrap();
        assert!(queue.ready(now + Duration::from_secs(30)));
    }

    #[test]
    fn edits_during_a_request_survive_and_disabling_cancels_them() {
        let now = Instant::now();
        let mut queue = Queue::new(true);
        queue.changed(now).unwrap();
        queue.started();
        queue.changed(now).unwrap();
        assert!(queue.ready(now + Duration::from_secs(30)));
        queue.configure(false);
        assert!(!queue.ready(now + Duration::from_secs(86400)));
    }
}
