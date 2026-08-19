//! Rule-agnostic presentation flow shared by every mahjong runtime.
//!
//! A rules engine should decide tiles, legal actions, scoring, and hand/match
//! transitions. Loading assets and waiting for every client to finish the
//! opening animation are application concerns, so they live here instead of
//! being copied into every rules implementation.

use crate::presentation::{
    ANIMATION_REPORT_GRACE_MS, MATCH_ASSET_LOAD_TIMEOUT_MS, OPENING_READY_FALLBACK_MS,
    SETTLEMENT_CONFIRM_MS,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReadyReport {
    AlreadyReported,
    Reported,
    EveryoneReady,
}

impl ReadyReport {
    #[must_use]
    pub(crate) const fn changed(self) -> bool {
        !matches!(self, Self::AlreadyReported)
    }

    #[must_use]
    pub(crate) const fn everyone_ready(self) -> bool {
        matches!(self, Self::EveryoneReady)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MatchOpening {
    assets_ready: Box<[bool]>,
    opening_ready: Box<[bool]>,
    assets_started_at_ms: u64,
    opening_started_at_ms: u64,
    first_opening_ready_at_ms: Option<u64>,
    terminated_by_asset_timeout: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SettlementFlow {
    settled_at_ms: u64,
    played: Box<[bool]>,
    confirm_started_at_ms: Option<u64>,
    confirmed: Box<[bool]>,
}

impl SettlementFlow {
    #[must_use]
    pub(crate) fn new(seat_count: usize, settled_at_ms: u64) -> Self {
        Self {
            settled_at_ms,
            played: vec![false; seat_count].into_boxed_slice(),
            confirm_started_at_ms: None,
            confirmed: vec![false; seat_count].into_boxed_slice(),
        }
    }

    #[must_use]
    pub(crate) fn played_flags(&self) -> &[bool] {
        &self.played
    }

    #[must_use]
    pub(crate) fn confirmed_flags(&self) -> &[bool] {
        &self.confirmed
    }

    #[must_use]
    pub(crate) const fn confirmation_open(&self) -> bool {
        self.confirm_started_at_ms.is_some()
    }

    #[must_use]
    pub(crate) fn confirm_deadline_ms(&self) -> Option<u64> {
        self.confirm_started_at_ms
            .map(|started_ms| started_ms.saturating_add(SETTLEMENT_CONFIRM_MS))
    }

    pub(crate) fn report_played(&mut self, seat: usize, now_ms: u64) -> ReadyReport {
        if self.played[seat] {
            return ReadyReport::AlreadyReported;
        }
        self.played[seat] = true;
        if self.played.iter().all(|played| *played) {
            self.confirm_started_at_ms.get_or_insert(now_ms);
            ReadyReport::EveryoneReady
        } else {
            ReadyReport::Reported
        }
    }

    pub(crate) fn report_confirmed(&mut self, seat: usize) -> ReadyReport {
        if self.confirmed[seat] {
            return ReadyReport::AlreadyReported;
        }
        self.confirmed[seat] = true;
        if self.confirmed.iter().all(|confirmed| *confirmed) {
            ReadyReport::EveryoneReady
        } else {
            ReadyReport::Reported
        }
    }

    pub(crate) fn open_confirmation_if_due(
        &mut self,
        now_ms: u64,
        reveal_fallback_ms: u64,
    ) -> bool {
        if self.confirmation_open()
            || now_ms < self.settled_at_ms.saturating_add(reveal_fallback_ms)
        {
            return false;
        }
        self.played.fill(true);
        self.confirm_started_at_ms = Some(now_ms);
        true
    }

    #[must_use]
    pub(crate) fn advance_due(&self, now_ms: u64, total_fallback_ms: u64) -> bool {
        let confirm_due = self
            .confirm_started_at_ms
            .is_some_and(|started_ms| now_ms.saturating_sub(started_ms) >= SETTLEMENT_CONFIRM_MS);
        confirm_due || now_ms.saturating_sub(self.settled_at_ms) >= total_fallback_ms
    }
}

/// 换三张阶段：每家选 3 张同花色牌，四家都提交后引擎立即交换，这里再等各家
/// 播完交换动画才放行到定缺。动画门要独立于「提交」：引擎在第 4 家提交时就当场
/// 换好了，视图得等动画播完才把新牌露给玩家。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExchangeFlow {
    submitted: Box<[bool]>,
    submitted_tile_ids: Box<[Option<[u16; 3]>]>,
    animation_played: Box<[bool]>,
    animation_started_at_ms: Option<u64>,
}

impl ExchangeFlow {
    #[must_use]
    pub(crate) fn new(seat_count: usize) -> Self {
        Self {
            submitted: vec![false; seat_count].into_boxed_slice(),
            submitted_tile_ids: vec![None; seat_count].into_boxed_slice(),
            animation_played: vec![false; seat_count].into_boxed_slice(),
            animation_started_at_ms: None,
        }
    }

    #[must_use]
    pub(crate) fn submitted_flags(&self) -> &[bool] {
        &self.submitted
    }

    #[must_use]
    pub(crate) fn animation_played_flags(&self) -> &[bool] {
        &self.animation_played
    }

    pub(crate) fn report_submitted(
        &mut self,
        seat: usize,
        tile_ids: [u16; 3],
        now_ms: u64,
    ) -> ReadyReport {
        if self.submitted[seat] {
            return ReadyReport::AlreadyReported;
        }
        self.submitted[seat] = true;
        self.submitted_tile_ids[seat] = Some(tile_ids);
        if self.submitted.iter().all(|submitted| *submitted) {
            self.animation_started_at_ms.get_or_insert(now_ms);
            ReadyReport::EveryoneReady
        } else {
            ReadyReport::Reported
        }
    }

    #[must_use]
    pub(crate) fn submitted_tile_ids(&self, seat: usize) -> Option<[u16; 3]> {
        self.submitted_tile_ids[seat]
    }

    pub(crate) fn report_animation_played(&mut self, seat: usize) -> ReadyReport {
        if self.animation_played[seat] {
            return ReadyReport::AlreadyReported;
        }
        self.animation_played[seat] = true;
        if self.animation_played.iter().all(|played| *played) {
            ReadyReport::EveryoneReady
        } else {
            ReadyReport::Reported
        }
    }

    /// 换三张动画超时兜底：四家都提交后，等不到动画回执就强制放行。
    #[must_use]
    pub(crate) fn release_due(&self, now_ms: u64, fallback_ms: u64) -> bool {
        self.animation_started_at_ms
            .is_some_and(|started_ms| now_ms.saturating_sub(started_ms) >= fallback_ms)
    }

    /// 兜底到期还没收齐动画回执，就把动画门整体放行并返回 `true`。
    pub(crate) fn release_if_due(&mut self, now_ms: u64, fallback_ms: u64) -> bool {
        if self.animation_played.iter().all(|played| *played)
            || !self.release_due(now_ms, fallback_ms)
        {
            return false;
        }
        self.animation_played.fill(true);
        true
    }
}

/// 定缺阶段：每家选一门要缺的花色，四家都提交后引擎立即开打。没有动画门。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DingQueFlow {
    submitted: Box<[bool]>,
}

impl DingQueFlow {
    #[must_use]
    pub(crate) fn new(seat_count: usize) -> Self {
        Self {
            submitted: vec![false; seat_count].into_boxed_slice(),
        }
    }

    #[must_use]
    pub(crate) fn submitted_flags(&self) -> &[bool] {
        &self.submitted
    }

    pub(crate) fn report_submitted(&mut self, seat: usize) -> ReadyReport {
        if self.submitted[seat] {
            return ReadyReport::AlreadyReported;
        }
        self.submitted[seat] = true;
        if self.submitted.iter().all(|submitted| *submitted) {
            ReadyReport::EveryoneReady
        } else {
            ReadyReport::Reported
        }
    }
}

impl MatchOpening {
    #[must_use]
    pub(crate) fn new(seat_count: usize, now_ms: u64) -> Self {
        Self {
            assets_ready: vec![false; seat_count].into_boxed_slice(),
            opening_ready: vec![false; seat_count].into_boxed_slice(),
            assets_started_at_ms: now_ms,
            opening_started_at_ms: now_ms,
            first_opening_ready_at_ms: None,
            terminated_by_asset_timeout: false,
        }
    }

    #[must_use]
    pub(crate) fn assets_loading(&self) -> bool {
        self.assets_ready.iter().any(|ready| !*ready)
    }

    #[must_use]
    pub(crate) fn opening_blocked(&self) -> bool {
        self.opening_ready.iter().any(|ready| !*ready)
    }

    #[must_use]
    pub(crate) const fn terminated_by_asset_timeout(&self) -> bool {
        self.terminated_by_asset_timeout
    }

    #[must_use]
    pub(crate) fn assets_ready_flags(&self) -> &[bool] {
        &self.assets_ready
    }

    #[must_use]
    pub(crate) fn opening_ready_flags(&self) -> &[bool] {
        &self.opening_ready
    }

    pub(crate) fn report_assets_ready(&mut self, seat: usize, now_ms: u64) -> ReadyReport {
        if self.assets_ready[seat] {
            return ReadyReport::AlreadyReported;
        }
        self.assets_ready[seat] = true;
        if self.assets_ready.iter().all(|ready| *ready) {
            self.opening_started_at_ms = now_ms;
            ReadyReport::EveryoneReady
        } else {
            ReadyReport::Reported
        }
    }

    pub(crate) fn report_opening_ready(&mut self, seat: usize, now_ms: u64) -> ReadyReport {
        if self.opening_ready[seat] {
            return ReadyReport::AlreadyReported;
        }
        self.opening_ready[seat] = true;
        self.first_opening_ready_at_ms.get_or_insert(now_ms);
        if self.opening_ready.iter().all(|ready| *ready) {
            ReadyReport::EveryoneReady
        } else {
            ReadyReport::Reported
        }
    }

    pub(crate) fn reset_hand(&mut self, now_ms: u64) {
        self.opening_ready.fill(false);
        self.opening_started_at_ms = now_ms;
        self.first_opening_ready_at_ms = None;
    }

    pub(crate) fn release_opening_if_due(&mut self, now_ms: u64) -> bool {
        if !self.opening_blocked() || !self.opening_ready_deadline_passed(now_ms) {
            return false;
        }
        self.opening_ready.fill(true);
        true
    }

    pub(crate) fn terminate_if_assets_stalled(&mut self, now_ms: u64) -> bool {
        if !self.assets_loading()
            || now_ms.saturating_sub(self.assets_started_at_ms) < MATCH_ASSET_LOAD_TIMEOUT_MS
        {
            return false;
        }
        self.terminated_by_asset_timeout = true;
        true
    }

    fn opening_ready_deadline_passed(&self, now_ms: u64) -> bool {
        let deadline = match self.first_opening_ready_at_ms {
            Some(first_ready_ms) => first_ready_ms.saturating_add(ANIMATION_REPORT_GRACE_MS),
            None => self
                .opening_started_at_ms
                .saturating_add(OPENING_READY_FALLBACK_MS),
        };
        now_ms >= deadline
    }
}

#[cfg(test)]
mod tests {
    use super::{MatchOpening, ReadyReport, SettlementFlow};
    use crate::presentation::{
        ANIMATION_REPORT_GRACE_MS, MATCH_ASSET_LOAD_TIMEOUT_MS, OPENING_READY_FALLBACK_MS,
        SETTLEMENT_CONFIRM_MS,
    };

    #[test]
    fn assets_and_opening_have_independent_gates() {
        let mut opening = MatchOpening::new(2, 100);
        assert!(opening.assets_loading());
        assert!(opening.opening_blocked());
        assert_eq!(opening.report_assets_ready(0, 200), ReadyReport::Reported);
        assert_eq!(
            opening.report_assets_ready(1, 300),
            ReadyReport::EveryoneReady
        );
        assert!(!opening.assets_loading());
        assert!(opening.opening_blocked());
        assert!(!opening.release_opening_if_due(300 + OPENING_READY_FALLBACK_MS - 1));
        assert!(opening.release_opening_if_due(300 + OPENING_READY_FALLBACK_MS));
    }

    #[test]
    fn first_opening_report_starts_short_grace_period() {
        let mut opening = MatchOpening::new(2, 0);
        opening.report_assets_ready(0, 10);
        opening.report_assets_ready(1, 10);
        assert_eq!(opening.report_opening_ready(0, 20), ReadyReport::Reported);
        assert!(!opening.release_opening_if_due(20 + ANIMATION_REPORT_GRACE_MS - 1));
        assert!(opening.release_opening_if_due(20 + ANIMATION_REPORT_GRACE_MS));
    }

    #[test]
    fn asset_timeout_is_reported_once() {
        let mut opening = MatchOpening::new(2, 50);
        assert!(!opening.terminate_if_assets_stalled(50 + MATCH_ASSET_LOAD_TIMEOUT_MS - 1));
        assert!(opening.terminate_if_assets_stalled(50 + MATCH_ASSET_LOAD_TIMEOUT_MS));
        assert!(opening.terminated_by_asset_timeout());
    }

    #[test]
    fn settlement_opens_confirmation_after_every_player_finishes() {
        let mut settlement = SettlementFlow::new(2, 100);
        assert_eq!(settlement.report_played(0, 200), ReadyReport::Reported);
        assert!(!settlement.confirmation_open());
        assert_eq!(settlement.report_played(1, 250), ReadyReport::EveryoneReady);
        assert_eq!(
            settlement.confirm_deadline_ms(),
            Some(250 + SETTLEMENT_CONFIRM_MS)
        );
        assert_eq!(settlement.report_confirmed(0), ReadyReport::Reported);
        assert_eq!(settlement.report_confirmed(1), ReadyReport::EveryoneReady);
    }

    #[test]
    fn settlement_fallback_can_open_and_advance_without_reports() {
        let mut settlement = SettlementFlow::new(2, 1_000);
        assert!(!settlement.open_confirmation_if_due(1_499, 500));
        assert!(settlement.open_confirmation_if_due(1_500, 500));
        assert!(settlement.played_flags().iter().all(|played| *played));
        assert!(!settlement.advance_due(1_500 + SETTLEMENT_CONFIRM_MS - 1, 20_000));
        assert!(settlement.advance_due(1_500 + SETTLEMENT_CONFIRM_MS, 20_000));
    }
}
