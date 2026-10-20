use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const MAX_FAILURES: u32 = 10;
const WINDOW: Duration = Duration::from_secs(15 * 60);

#[derive(Default)]
pub struct LoginThrottle(Mutex<HashMap<String, (u32, Instant)>>);

impl LoginThrottle {
    pub fn is_locked(&self, email: &str) -> bool {
        let failures = self.0.lock().unwrap();
        matches!(failures.get(email), Some(&(count, since)) if count >= MAX_FAILURES && since.elapsed() < WINDOW)
    }

    pub fn record_failure(&self, email: &str) {
        let mut failures = self.0.lock().unwrap();
        failures.retain(|_, (_, since)| since.elapsed() < WINDOW);
        failures
            .entry(email.to_owned())
            .or_insert((0, Instant::now()))
            .0 += 1;
    }

    pub fn clear(&self, email: &str) {
        self.0.lock().unwrap().remove(email);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locks_after_repeated_failures_and_clears_on_success() {
        let throttle = LoginThrottle::default();
        for _ in 0..MAX_FAILURES {
            assert!(!throttle.is_locked("a@b.c"));
            throttle.record_failure("a@b.c");
        }
        assert!(throttle.is_locked("a@b.c"));
        assert!(!throttle.is_locked("other@b.c"));
        throttle.clear("a@b.c");
        assert!(!throttle.is_locked("a@b.c"));
    }
}
