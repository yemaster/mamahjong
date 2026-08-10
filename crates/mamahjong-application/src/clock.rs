use mahjong_core::{MatchId, UserId};

/// Thinking time granted again for every decision.
pub const BASE_THINKING_MS: u64 = 5_000;

/// Default extra thinking time shared by all decisions of one seat in one hand.
pub const RESERVE_THINKING_MS: u32 = 20_000;

/// One seat's thinking time, measured on the caller's monotonic millisecond clock.
///
/// A seat is "on the clock" while it owes a decision. The reserve pool only
/// drains, so a match always ends within a bounded amount of time.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeatClock {
    armed_at_ms: Option<u64>,
    base_ms: u64,
    reserve_ms: u32,
}

impl SeatClock {
    #[must_use]
    #[cfg(test)]
    pub(crate) const fn new() -> Self {
        Self {
            armed_at_ms: None,
            base_ms: BASE_THINKING_MS,
            reserve_ms: RESERVE_THINKING_MS,
        }
    }

    #[must_use]
    pub(crate) const fn with_limits(base_ms: u64, reserve_ms: u32) -> Self {
        Self {
            armed_at_ms: None,
            base_ms,
            reserve_ms,
        }
    }

    /// When the current decision started, or `None` when the seat is idle.
    #[must_use]
    pub const fn armed_at_ms(&self) -> Option<u64> {
        self.armed_at_ms
    }

    /// Reserve left after the base time of the current decision runs out.
    #[must_use]
    pub const fn reserve_ms(&self) -> u32 {
        self.reserve_ms
    }

    /// Base time restored for every decision.
    #[must_use]
    pub const fn base_ms(&self) -> u64 {
        self.base_ms
    }

    /// The instant at which this seat's decision is taken over by the server.
    #[must_use]
    pub const fn deadline_ms(&self) -> Option<u64> {
        match self.armed_at_ms {
            Some(armed_at) => Some(armed_at + self.base_ms + self.reserve_ms as u64),
            None => None,
        }
    }

    /// Starts the clock; a seat that is already waiting keeps its start time.
    ///
    /// `start_ms` may sit in the future: that is how the animation grace of the
    /// command that just landed (see [`crate::presentation`]) is kept off the
    /// next decision's thinking time. Everything downstream tolerates it — the
    /// deadline simply moves along, nothing has elapsed yet, and a seat that
    /// answers during the grace is charged no reserve at all.
    pub(crate) const fn arm(&mut self, start_ms: u64) {
        if self.armed_at_ms.is_none() {
            self.armed_at_ms = Some(start_ms);
        }
    }

    /// Freezes an armed clock without charging reserve time.
    ///
    /// Negative when the clock was still waiting out an animation, so that the
    /// rest of that wait survives the pause instead of being handed to the
    /// player as thinking time.
    pub(crate) fn pause(&mut self, now_ms: u64) -> Option<i64> {
        self.armed_at_ms.take().map(|armed_at| {
            let now = i64::try_from(now_ms).unwrap_or(i64::MAX);
            let armed_at = i64::try_from(armed_at).unwrap_or(i64::MAX);
            now.saturating_sub(armed_at)
        })
    }

    /// Restores the exact elapsed duration captured by [`Self::pause`].
    pub(crate) fn resume(&mut self, now_ms: u64, elapsed_ms: Option<i64>) {
        self.armed_at_ms = elapsed_ms.map(|elapsed| {
            if elapsed >= 0 {
                now_ms.saturating_sub(elapsed.unsigned_abs())
            } else {
                now_ms.saturating_add(elapsed.unsigned_abs())
            }
        });
    }

    /// Stops the clock and charges everything beyond the base time to the reserve.
    pub(crate) fn disarm(&mut self, now_ms: u64) {
        let Some(armed_at) = self.armed_at_ms.take() else {
            return;
        };
        let overrun = now_ms.saturating_sub(armed_at).saturating_sub(self.base_ms);
        let overrun = u32::try_from(overrun).unwrap_or(u32::MAX);
        self.reserve_ms = self.reserve_ms.saturating_sub(overrun);
    }

    #[must_use]
    pub(crate) const fn expired(&self, now_ms: u64) -> bool {
        match self.deadline_ms() {
            Some(deadline) => now_ms >= deadline,
            None => false,
        }
    }
}

/// A match the clock sweeper advanced on behalf of a seat.
///
/// `actor` is the seat the timeout acted for; it is a match player, so callers
/// can reuse it for the reads and archives a manual command would perform.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClockExpiry {
    pub match_id: MatchId,
    pub actor: UserId,
    pub version: u64,
    pub latest_sequence: u64,
    pub finished: bool,
}

#[cfg(test)]
mod tests {
    use super::{BASE_THINKING_MS, RESERVE_THINKING_MS, SeatClock};

    #[test]
    fn an_idle_seat_has_no_deadline() {
        let clock = SeatClock::new();

        assert_eq!(clock.deadline_ms(), None);
        assert!(!clock.expired(u64::MAX));
    }

    #[test]
    fn the_deadline_covers_the_base_time_and_the_reserve() {
        let mut clock = SeatClock::new();
        clock.arm(1_000);

        assert_eq!(
            clock.deadline_ms(),
            Some(1_000 + BASE_THINKING_MS + u64::from(RESERVE_THINKING_MS))
        );
        assert!(!clock.expired(clock.deadline_ms().expect("deadline") - 1));
        assert!(clock.expired(clock.deadline_ms().expect("deadline")));
    }

    #[test]
    fn deciding_within_the_base_time_costs_no_reserve() {
        let mut clock = SeatClock::new();
        clock.arm(1_000);
        clock.disarm(1_000 + BASE_THINKING_MS);

        assert_eq!(clock.reserve_ms(), RESERVE_THINKING_MS);
        assert_eq!(clock.armed_at_ms(), None);
    }

    #[test]
    fn only_the_overrun_drains_the_reserve() {
        let mut clock = SeatClock::new();
        clock.arm(1_000);
        clock.disarm(1_000 + BASE_THINKING_MS + 3_000);

        assert_eq!(clock.reserve_ms(), RESERVE_THINKING_MS - 3_000);
    }

    #[test]
    fn a_drained_reserve_leaves_only_the_base_time() {
        let mut clock = SeatClock::new();
        clock.arm(0);
        clock.disarm(BASE_THINKING_MS + u64::from(RESERVE_THINKING_MS) * 2);

        assert_eq!(clock.reserve_ms(), 0);
        clock.arm(100);
        assert_eq!(clock.deadline_ms(), Some(100 + BASE_THINKING_MS));
    }

    #[test]
    fn rearming_a_waiting_seat_keeps_its_start_time() {
        let mut clock = SeatClock::new();
        clock.arm(1_000);
        clock.arm(4_000);

        assert_eq!(clock.armed_at_ms(), Some(1_000));
    }

    #[test]
    fn a_clock_armed_behind_an_animation_does_not_run_yet() {
        let grace = 1_420;
        let mut clock = SeatClock::new();
        clock.arm(1_000 + grace);

        // 动画播完之前不会超时，之后才是完整的思考时间。
        assert!(!clock.expired(1_000 + grace - 1));
        assert_eq!(
            clock.deadline_ms(),
            Some(1_000 + grace + BASE_THINKING_MS + u64::from(RESERVE_THINKING_MS))
        );
    }

    #[test]
    fn answering_during_the_animation_costs_no_reserve() {
        let grace = 1_420;
        let mut clock = SeatClock::new();
        clock.arm(1_000 + grace);
        clock.disarm(1_000 + grace / 2);

        assert_eq!(clock.reserve_ms(), RESERVE_THINKING_MS);
        assert_eq!(clock.armed_at_ms(), None);
    }

    #[test]
    fn pausing_during_an_animation_keeps_the_rest_of_the_wait() {
        let grace = 1_420;
        let mut clock = SeatClock::new();
        clock.arm(1_000 + grace);

        // 退出投票在动画播到一半时把时钟冻住。
        let paused = clock.pause(1_000 + grace / 2);
        assert_eq!(paused, Some(-(grace as i64) / 2));
        clock.resume(9_000, paused);

        assert_eq!(clock.armed_at_ms(), Some(9_000 + grace / 2));
    }

    #[test]
    fn a_seat_already_waiting_keeps_its_start_time_through_an_animation() {
        let mut clock = SeatClock::new();
        clock.arm(1_000);
        clock.arm(2_000 + 1_420);

        assert_eq!(clock.armed_at_ms(), Some(1_000));
    }

    #[test]
    fn two_seats_on_the_clock_drain_their_reserves_independently() {
        let mut seat0 = SeatClock::new();
        let mut seat1 = SeatClock::new();
        seat0.arm(1_000);
        seat1.arm(1_000);

        // Only seat 0 runs out of base time.
        seat0.disarm(1_000 + BASE_THINKING_MS + 3_000);
        assert_eq!(seat0.reserve_ms(), RESERVE_THINKING_MS - 3_000);

        // Seat 1 is unaffected — its reserve is still full.
        assert_eq!(seat1.reserve_ms(), RESERVE_THINKING_MS);
        assert!(seat1.armed_at_ms().is_some());

        // Later seat 1 expires too.
        seat1.disarm(1_000 + BASE_THINKING_MS + 7_000);
        assert_eq!(seat1.reserve_ms(), RESERVE_THINKING_MS - 7_000);
    }

    #[test]
    fn configured_limits_drive_deadline_and_reserve_charging() {
        let mut clock = SeatClock::with_limits(15_000, 60_000);
        clock.arm(2_000);
        assert_eq!(clock.deadline_ms(), Some(77_000));
        clock.disarm(20_000);
        assert_eq!(clock.base_ms(), 15_000);
        assert_eq!(clock.reserve_ms(), 57_000);
    }

    #[test]
    fn zero_reserve_expires_after_base_time() {
        let mut clock = SeatClock::with_limits(5_000, 0);
        clock.arm(100);
        assert_eq!(clock.deadline_ms(), Some(5_100));
    }
}
