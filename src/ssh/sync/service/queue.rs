use std::time::{Duration, Instant};

const SETTLE: Duration = Duration::from_secs(2);

#[derive(Default)]
pub(super) struct Queue {
    due: Option<Instant>,
    failures: usize,
}

impl Queue {
    pub fn changed(&mut self, now: Instant) {
        self.due = Some(now + SETTLE);
        self.failures = 0;
    }

    pub fn ready(&self, now: Instant) -> bool {
        self.due.is_some_and(|due| now >= due)
    }

    pub fn started(&mut self) {
        self.due = None;
    }

    pub fn finished(&mut self, success: bool, now: Instant) {
        if success {
            self.failures = 0;
        } else if self.due.is_none() {
            if let Some(delay) = [30, 120, 600].get(self.failures) {
                self.due = Some(now + Duration::from_secs(*delay));
            }
            self.failures += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rapid_edits_debounce_and_a_consumed_request_does_not_repeat() {
        let mut queue = Queue::default();
        let now = Instant::now();
        queue.changed(now);
        queue.changed(now + Duration::from_secs(1));
        assert!(!queue.ready(now + SETTLE));
        assert!(queue.ready(now + SETTLE + Duration::from_secs(1)));
        queue.started();
        queue.finished(true, now);
        assert!(!queue.ready(now + Duration::from_secs(100)));
    }

    #[test]
    fn changes_during_a_request_survive_its_completion() {
        let mut queue = Queue::default();
        let now = Instant::now();
        queue.changed(now);
        queue.started();
        queue.changed(now);
        queue.finished(true, now);
        assert!(queue.ready(now + SETTLE));
    }

    #[test]
    fn failures_retry_with_a_bound_and_new_changes_reset_the_budget() {
        let mut queue = Queue::default();
        let mut now = Instant::now();
        for delay in [30, 120, 600] {
            queue.started();
            queue.finished(false, now);
            assert!(!queue.ready(now + Duration::from_secs(delay - 1)));
            now += Duration::from_secs(delay);
            assert!(queue.ready(now));
        }
        queue.started();
        queue.finished(false, now);
        assert!(!queue.ready(now + Duration::from_secs(3600)));
        queue.changed(now);
        assert!(queue.ready(now + SETTLE));
    }

    #[test]
    fn successful_partial_syncs_preserve_edits_without_scheduling_retries() {
        let mut queue = Queue::default();
        let now = Instant::now();
        queue.changed(now);
        queue.started();
        queue.finished(true, now);
        assert!(!queue.ready(now + Duration::from_secs(3600)));
        queue.changed(now);
        queue.started();
        queue.changed(now);
        queue.finished(true, now);
        assert!(queue.ready(now + SETTLE));
    }
}
