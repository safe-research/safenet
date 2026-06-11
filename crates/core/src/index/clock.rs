//! A wall clock implementation that produces Unix epoch timestamps and is
//! compatible with `tokio::time::{advance, pause}` in tests.

#![allow(dead_code)]

use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Estimates the current wall-clock time from elapsed monotonic time.
#[derive(Clone, Debug)]
pub struct Clock {
    base_ms: u64,
    anchor: tokio::time::Instant,
}

impl Clock {
    /// Anchors "now" to the current system time.
    pub fn start() -> Self {
        let base_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        Self {
            base_ms,
            anchor: tokio::time::Instant::now(),
        }
    }

    /// The estimated current wall-clock time, in milliseconds.
    pub fn now_ms(&self) -> u64 {
        self.base_ms + self.anchor.elapsed().as_millis() as u64
    }

    /// Sleeps until a target Unix epoch time, in milliseconds.
    ///
    /// If `target` is in the past, returns immediately.
    pub async fn sleep_until(&self, target: u64) {
        let now = self.now_ms();
        if target > now {
            tokio::time::sleep(Duration::from_millis(target - now)).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn tokio_test_util_compatibility() {
        let clock = Clock::start();
        let start = clock.now_ms();
        let target = start + 1_000;

        let sleeper = tokio::spawn({
            let clock = clock.clone();
            async move {
                clock.sleep_until(target).await;
            }
        });

        tokio::task::yield_now().await;
        assert!(!sleeper.is_finished());

        tokio::time::advance(Duration::from_millis(999)).await;
        assert_eq!(clock.now_ms(), start + 999);
        assert!(!sleeper.is_finished());

        tokio::time::advance(Duration::from_millis(1)).await;
        sleeper.await.unwrap();
        assert_eq!(clock.now_ms(), target);
    }

    #[tokio::test(start_paused = true)]
    async fn returns_instantly_when_sleeping_until_past() {
        let clock = Clock::start();
        let start = clock.now_ms();

        // Tokio time is paused, this would never return if we were waiting for
        // some amount of time to elapse.
        clock.sleep_until(start - 1).await;
        assert_eq!(clock.now_ms(), start);
    }
}
