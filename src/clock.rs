use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// The current time, read once per event-loop iteration and passed to
/// `update` and `render`, so neither reads the real clock itself.
///
/// Tests build a fixed clock so rendered output and saved timestamps are
/// deterministic.
#[derive(Clone, Copy, Debug)]
pub struct Clock {
    /// Wall-clock time in milliseconds since the Unix epoch, used for task
    /// timestamps and relative times such as "5m ago".
    pub wall_millis: i64,
    /// Monotonic time, used for animations and toast expiry.
    pub instant: Instant,
}

impl Clock {
    pub fn now() -> Self {
        Self {
            wall_millis: now_millis(),
            instant: Instant::now(),
        }
    }

    pub fn fixed(wall_millis: i64) -> Self {
        Self {
            wall_millis,
            instant: Instant::now(),
        }
    }

    /// The same clock moved forward by `duration`.
    pub fn advance(self, duration: std::time::Duration) -> Self {
        Self {
            wall_millis: self
                .wall_millis
                .saturating_add(duration.as_millis().try_into().unwrap_or(i64::MAX)),
            instant: self.instant + duration,
        }
    }
}

pub fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn timestamp_is_non_negative() {
        assert!(now_millis() >= 0);
    }

    #[test]
    fn advance_moves_both_clocks() {
        let clock = Clock::fixed(1_000);
        let later = clock.advance(Duration::from_millis(250));
        assert_eq!(later.wall_millis, 1_250);
        assert_eq!(later.instant - clock.instant, Duration::from_millis(250));
    }
}
