//! A wall clock that produces Unix epoch timestamps.
//!
//! In production it reads the system clock directly. In tests it is instead
//! driven by `tokio`'s clock, anchored at [`TEST_SYSTEM_TIME_EPOCH_SECONDS`], so
//! it stays compatible with `tokio::time::{advance, pause}`.

use std::time::Duration;
#[cfg(not(test))]
use std::time::{SystemTime, UNIX_EPOCH};
#[cfg(test)]
use tokio::time::Instant;

/// A source of the current wall-clock time, in Unix epoch milliseconds.
#[derive(Clone, Debug)]
pub struct Clock {
    /// The monotonic instant the clock was started at. "Now" is derived as the
    /// time elapsed since this instant added to [`TEST_SYSTEM_TIME_EPOCH_SECONDS`].
    #[cfg(test)]
    anchor: Instant,
}

impl Clock {
    /// Starts a new clock.
    pub fn start() -> Self {
        Self {
            #[cfg(test)]
            anchor: Instant::now(),
        }
    }

    /// The current wall-clock time, in Unix epoch milliseconds.
    #[cfg(not(test))]
    pub fn now_ms(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    /// The current wall-clock time, in Unix epoch milliseconds.
    ///
    /// This is [`TEST_SYSTEM_TIME_EPOCH_SECONDS`] plus the `tokio` time elapsed
    /// since the clock was started, so it advances with `tokio::time::advance`.
    #[cfg(test)]
    pub fn now_ms(&self) -> u64 {
        TEST_SYSTEM_TIME_EPOCH_SECONDS * 1_000 + self.anchor.elapsed().as_millis() as u64
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

/// The reference system time used when testing.
#[cfg(test)]
pub const TEST_SYSTEM_TIME_EPOCH_SECONDS: u64 = 1_337_420;

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
