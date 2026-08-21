use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use mamahjong_application::SeatClock;
use serde::Serialize;

use crate::AppState;

/// How often expired seats are swept.
///
/// One shared task instead of one timer per match: the number of live matches
/// in a process is bounded, and `expire_clocks` is a pure function of `now_ms`,
/// so tests feed it time directly instead of waiting.
pub(crate) const SWEEP_INTERVAL: Duration = Duration::from_millis(200);
const OFFLINE_ACTION_DELAY: Duration = Duration::from_secs(1);

/// The single source of time for every seat clock in this process.
///
/// Millisecond offsets from process start, so the coordinate system survives
/// wall-clock adjustments and can be handed to the application layer as `u64`.
#[derive(Clone, Debug)]
pub(crate) struct MonotonicClock {
    origin: Instant,
    /// Test-only offset; production always reads zero.
    skew_ms: Arc<AtomicU64>,
}

impl MonotonicClock {
    #[must_use]
    pub(crate) fn new() -> Self {
        Self {
            origin: Instant::now(),
            skew_ms: Arc::new(AtomicU64::new(0)),
        }
    }

    #[must_use]
    pub(crate) fn now_ms(&self) -> u64 {
        let elapsed = u64::try_from(self.origin.elapsed().as_millis()).unwrap_or(u64::MAX);
        elapsed.saturating_add(self.skew_ms.load(Ordering::Relaxed))
    }

    /// Jumps the clock forward so tests reach a deadline without sleeping.
    #[cfg(test)]
    pub(crate) fn advance(&self, millis: u64) {
        self.skew_ms.fetch_add(millis, Ordering::Relaxed);
    }
}

impl Default for MonotonicClock {
    fn default() -> Self {
        Self::new()
    }
}

/// One seat's countdown as clients render it.
///
/// The application layer stores absolute instants; the transport turns them
/// into the remaining time so no client has to know the server's origin.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct SeatCountdown {
    pub(crate) seat: u8,
    pub(crate) remaining_ms: u64,
    pub(crate) base_ms: u64,
    pub(crate) reserve_ms: u64,
}

impl SeatCountdown {
    /// Countdowns of the seats on the clock, in seat order; idle seats are omitted.
    #[must_use]
    pub(crate) fn snapshot(clocks: &[SeatClock], now_ms: u64) -> Vec<Self> {
        clocks
            .iter()
            .enumerate()
            .filter_map(|(index, clock)| {
                let armed_at_ms = clock.armed_at_ms()?;
                // 上一步操作的动画还没播完时时钟是往后上弦的，这段等待要一并
                // 算进剩余时间里，客户端本地插值出来的秒数才和服务端一致。
                let pending_ms = armed_at_ms.saturating_sub(now_ms);
                let elapsed = now_ms.saturating_sub(armed_at_ms);
                let configured_base_ms = clock.base_ms();
                let base_ms = configured_base_ms.saturating_sub(elapsed) + pending_ms;
                let reserve_ms = u64::from(clock.reserve_ms())
                    .saturating_sub(elapsed.saturating_sub(configured_base_ms));
                Some(Self {
                    seat: u8::try_from(index).unwrap_or(u8::MAX),
                    remaining_ms: base_ms + reserve_ms,
                    base_ms,
                    reserve_ms,
                })
            })
            .collect()
    }
}

/// Plays the timeout action for every seat that ran out of time.
///
/// Each expiry is finished exactly like a manual command, so clients cannot
/// tell a timeout from a played tile.
pub(crate) async fn sweep(state: &AppState) {
    sweep_offline_players(state).await;
    let expiries = match state.application().expire_clocks(state.now_ms()) {
        Ok(expiries) => expiries,
        Err(error) => {
            tracing::error!(code = ?error.code(), "座位时钟扫描失败");
            return;
        }
    };
    for expiry in expiries {
        tracing::debug!(match_id = %expiry.match_id, "超时自动推进");
        let _ = crate::api::announce_advance(
            state,
            &expiry.actor,
            &expiry.match_id,
            expiry.version,
            expiry.latest_sequence,
            expiry.finished,
        )
        .await;
    }
}

async fn sweep_offline_players(state: &AppState) {
    for (stream, actor) in state.realtime().offline_users() {
        let Some(match_id) = crate::api::parse_match_stream(&stream) else {
            continue;
        };
        let Some(allow_action) =
            state
                .realtime()
                .offline_action_ready(&stream, &actor, OFFLINE_ACTION_DELAY)
        else {
            continue;
        };
        let advance = match state.application().automate_player(
            &actor,
            &match_id,
            state.now_ms(),
            allow_action,
        ) {
            Ok(advance) => advance,
            Err(error) => {
                tracing::error!(
                    code = ?error.code(),
                    %match_id,
                    actor = %actor,
                    "离线托管推进失败"
                );
                continue;
            }
        };
        let Some(advance) = advance else {
            continue;
        };
        let _ = crate::api::announce_advance(
            state,
            &advance.actor,
            &advance.match_id,
            advance.version,
            advance.latest_sequence,
            advance.finished,
        )
        .await;
    }
}

/// Runs [`sweep`] until the returned handle is dropped.
pub fn spawn_sweeper(state: AppState) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(SWEEP_INTERVAL);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            sweep(&state).await;
        }
    })
}

#[cfg(test)]
mod tests {
    use mamahjong_application::{BASE_THINKING_MS, RESERVE_THINKING_MS, SeatClock};

    use super::{AppState, MonotonicClock, SeatCountdown};

    #[test]
    fn the_clock_only_moves_forward() {
        let clock = MonotonicClock::new();
        let first = clock.now_ms();
        clock.advance(25_000);
        let second = clock.now_ms();

        assert!(second >= first + 25_000);
        assert!(clock.now_ms() >= second);
    }

    #[test]
    fn clones_share_the_same_coordinate_system() {
        let clock = MonotonicClock::new();
        let clone = clock.clone();
        clock.advance(5_000);

        assert!(clone.now_ms() >= 5_000);
    }

    /// The clocks of a freshly dealt table, where only the dealer is waiting.
    fn dealt_clocks(state: &AppState) -> (Vec<SeatClock>, u64) {
        let registered = crate::testing::players(state, "countdown", 3);
        let seated: Vec<_> = registered.iter().map(|(user, _)| user.clone()).collect();
        let match_id = crate::testing::sanma_match(state, &seated);
        let view = state
            .application()
            .match_view(seated[0].id(), &match_id)
            .expect("view");
        let armed_at = view
            .clocks()
            .iter()
            .find_map(SeatClock::armed_at_ms)
            .expect("the dealer is on the clock");
        (view.clocks().to_vec(), armed_at)
    }

    #[test]
    fn only_waiting_seats_get_a_countdown() {
        let (clocks, armed_at) = dealt_clocks(&AppState::new());

        let seats = SeatCountdown::snapshot(&clocks, armed_at);
        assert_eq!(seats.len(), 1, "a fresh hand waits for the dealer alone");
        assert_eq!(seats[0].base_ms, BASE_THINKING_MS);
        assert_eq!(seats[0].reserve_ms, u64::from(RESERVE_THINKING_MS));
        assert_eq!(
            seats[0].remaining_ms,
            BASE_THINKING_MS + u64::from(RESERVE_THINKING_MS)
        );
    }

    #[test]
    fn the_base_time_drains_before_the_reserve() {
        let (clocks, armed_at) = dealt_clocks(&AppState::new());

        let early = SeatCountdown::snapshot(&clocks, armed_at + 2_000);
        assert_eq!(early[0].base_ms, BASE_THINKING_MS - 2_000);
        assert_eq!(early[0].reserve_ms, u64::from(RESERVE_THINKING_MS));

        let late = SeatCountdown::snapshot(&clocks, armed_at + BASE_THINKING_MS + 3_000);
        assert_eq!(late[0].base_ms, 0);
        assert_eq!(late[0].reserve_ms, u64::from(RESERVE_THINKING_MS) - 3_000);
        assert_eq!(late[0].remaining_ms, late[0].reserve_ms);
    }

    /// 动画还没播完时读秒里含着这段等待，客户端本地插值才不会和服务端错开。
    #[test]
    fn a_pending_animation_is_folded_into_the_countdown() {
        let grace = 1_420;
        let state = AppState::new();
        // 先把时钟推过一段动画时长，好让「上弦时刻在未来」有得可减。
        state.advance_clock(grace);
        let (clocks, armed_at) = dealt_clocks(&state);
        assert!(armed_at >= grace);

        let during = SeatCountdown::snapshot(&clocks, armed_at - grace);
        assert_eq!(during[0].base_ms, BASE_THINKING_MS + grace);
        assert_eq!(during[0].reserve_ms, u64::from(RESERVE_THINKING_MS));
        assert_eq!(
            during[0].remaining_ms,
            BASE_THINKING_MS + grace + u64::from(RESERVE_THINKING_MS)
        );

        // 客户端把这一帧本地倒推 grace 毫秒，正好等于动画播完那一刻的那一帧。
        let after = SeatCountdown::snapshot(&clocks, armed_at);
        assert_eq!(during[0].remaining_ms - grace, after[0].remaining_ms);
    }

    #[test]
    fn an_expired_seat_counts_down_to_zero_and_stays_there() {
        let (clocks, armed_at) = dealt_clocks(&AppState::new());

        let seats = SeatCountdown::snapshot(&clocks, armed_at + 3_600_000);
        assert_eq!(seats[0].remaining_ms, 0);
        assert_eq!(seats[0].base_ms, 0);
        assert_eq!(seats[0].reserve_ms, 0);
    }
}
