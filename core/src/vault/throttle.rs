//! Delay after failed unlock attempts (V-8): 1 s, 2 s, 4 s, … capped at 60 s.
//!
//! Kept in memory only (decision D-5): a persisted counter would be plaintext
//! metadata, and an offline attacker bypasses it anyway. The real protection
//! against guessing is the Argon2id cost. Data is never wiped automatically.

use std::time::{Duration, Instant};

const MAX_DELAY: Duration = Duration::from_secs(60);

#[derive(Debug, Default)]
pub struct UnlockThrottle {
    failures: u32,
    blocked_until: Option<Instant>,
}

impl UnlockThrottle {
    pub fn new() -> Self {
        Self::default()
    }

    /// `Err(remaining)` while the user has to wait before the next attempt.
    pub fn check(&self, now: Instant) -> Result<(), Duration> {
        match self.blocked_until {
            Some(until) if until > now => Err(until - now),
            _ => Ok(()),
        }
    }

    /// Records a failed attempt and returns the delay before the next one.
    pub fn record_failure(&mut self, now: Instant) -> Duration {
        self.failures = self.failures.saturating_add(1);
        let delay = delay_after(self.failures);
        self.blocked_until = Some(now + delay);
        delay
    }

    /// Resets the counter after a successful unlock.
    pub fn record_success(&mut self) {
        *self = Self::default();
    }
}

fn delay_after(failures: u32) -> Duration {
    let exp = failures.saturating_sub(1).min(6); // 2^6 = 64 > 60
    Duration::from_secs(1u64 << exp).min(MAX_DELAY)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delays_double_and_cap_at_sixty_seconds() {
        let secs: Vec<u64> = (1..=10).map(|n| delay_after(n).as_secs()).collect();
        assert_eq!(secs, [1, 2, 4, 8, 16, 32, 60, 60, 60, 60]);
        assert_eq!(delay_after(u32::MAX), MAX_DELAY);
    }

    #[test]
    fn blocks_until_delay_passes_and_resets_on_success() {
        let t0 = Instant::now();
        let mut throttle = UnlockThrottle::new();
        assert_eq!(throttle.check(t0), Ok(()));
        assert_eq!(throttle.record_failure(t0), Duration::from_secs(1));
        assert_eq!(throttle.check(t0), Err(Duration::from_secs(1)));
        assert_eq!(throttle.check(t0 + Duration::from_secs(1)), Ok(()));
        assert_eq!(
            throttle.record_failure(t0 + Duration::from_secs(1)),
            Duration::from_secs(2)
        );
        throttle.record_success();
        assert_eq!(throttle.check(t0 + Duration::from_secs(1)), Ok(()));
        assert_eq!(throttle.record_failure(t0), Duration::from_secs(1));
    }
}
