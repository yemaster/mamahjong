//! 两套规则引擎共用的运行时外壳。
//!
//! 房间里坐的是哪套规则，只在 `GameRuntime::start` 里决定一次。之后所有会话级
//! 操作都走 `RuleRuntime`，新增规则不需要再为每个操作给枚举补一条分派分支。

use mahjong_core::{MatchId, UserId};

use crate::clock::SeatClock;
use crate::game::{ObserverMatch, RiichiRuntime, SubmitGameCommand};
use crate::impact_game::{ImpactRuntime, ObserverImpactMatch};
use crate::room::GameRuleSnapshot;
use crate::stream::MatchEventPage;
use crate::{ApplicationError, ErrorCode, Room};

/// 一位观察者看到的牌桌，按规则家族分叉。
#[derive(Clone, Debug)]
pub enum MatchProjection {
    Riichi(Box<ObserverMatch>),
    Impact(Box<ObserverImpactMatch>),
}

impl MatchProjection {
    #[must_use]
    pub const fn variant_kind(&self) -> &'static str {
        match self {
            Self::Riichi(_) => "riichi",
            Self::Impact(_) => "impact",
        }
    }

    #[must_use]
    pub fn version(&self) -> u64 {
        match self {
            Self::Riichi(view) => view.version(),
            Self::Impact(view) => view.version,
        }
    }

    /// 事件游标。冲击麻将不生成事件流，这个数恒为 0。
    #[must_use]
    pub fn event_sequence(&self) -> u64 {
        match self {
            Self::Riichi(view) => view.event_sequence(),
            Self::Impact(view) => view.event_sequence,
        }
    }

    /// 整场是否已经打完（进结算页）。
    #[must_use]
    pub fn has_result(&self) -> bool {
        match self {
            Self::Riichi(view) => view.result().is_some(),
            Self::Impact(view) => view.result.is_some(),
        }
    }

    #[must_use]
    pub fn terminated_by_exit_vote(&self) -> bool {
        match self {
            Self::Riichi(view) => view.terminated_by_exit_vote(),
            Self::Impact(view) => view.terminated_by_exit_vote,
        }
    }

    /// 各座位的读秒。倒计时帧只认这个，不必知道桌上打的是哪套规则。
    #[must_use]
    pub fn clocks(&self) -> &[SeatClock] {
        match self {
            Self::Riichi(view) => view.clocks(),
            Self::Impact(view) => &view.clocks,
        }
    }

    /// 座位号与坐在上面的人，按座位顺序。在线状态帧照这个列。
    #[must_use]
    pub fn seated(&self) -> Vec<(u8, &UserId)> {
        match self {
            Self::Riichi(view) => view
                .players()
                .iter()
                .map(|player| (player.player().seat().index(), player.player().user_id()))
                .collect(),
            Self::Impact(view) => view
                .players
                .iter()
                .map(|player| (player.player.seat(), player.player.user_id()))
                .collect(),
        }
    }

    #[must_use]
    pub fn as_riichi(&self) -> Option<&ObserverMatch> {
        match self {
            Self::Riichi(view) => Some(view),
            Self::Impact(_) => None,
        }
    }

    #[must_use]
    pub fn as_impact(&self) -> Option<&ObserverImpactMatch> {
        match self {
            Self::Impact(view) => Some(view),
            Self::Riichi(_) => None,
        }
    }

    /// 只认立直投影的旧调用方走这条；冲击麻将会拿到一个明确的错误而不是被误读。
    ///
    /// # Errors
    ///
    /// 这局不是立直麻将。
    pub fn into_riichi(self) -> Result<ObserverMatch, ApplicationError> {
        match self {
            Self::Riichi(view) => Ok(*view),
            Self::Impact(_) => Err(not_riichi()),
        }
    }
}

trait RuleRuntime: std::fmt::Debug + Send + Sync {
    fn as_riichi(&self) -> Option<&RiichiRuntime> {
        None
    }

    fn generates_record(&self) -> bool {
        false
    }

    fn version(&self) -> u64;
    fn event_sequence(&self) -> u64;
    fn any_player(&self) -> Option<UserId>;
    fn projection(&self, actor: &UserId) -> Result<MatchProjection, ApplicationError>;
    fn events_after(
        &self,
        actor: &UserId,
        after_sequence: u64,
    ) -> Result<MatchEventPage, ApplicationError>;
    fn execute(
        &mut self,
        actor: &UserId,
        command: SubmitGameCommand,
        now_ms: u64,
    ) -> Result<(), ApplicationError>;
    fn is_finished(&self) -> bool;
    fn has_pending_settlement(&self) -> bool;
    fn terminate_if_assets_stalled(&mut self, now_ms: u64) -> Result<bool, ApplicationError>;
    fn advance_settlement_if_due(&mut self, now_ms: u64) -> Result<bool, ApplicationError>;
    fn open_settlement_confirm_if_due(&mut self, now_ms: u64) -> Result<bool, ApplicationError>;
    fn release_opening_if_due(&mut self, now_ms: u64) -> Result<bool, ApplicationError>;
    fn expire(&mut self, now_ms: u64) -> Result<Option<UserId>, ApplicationError>;
    fn set_dev_hand(&mut self, _actor: &UserId, _codes: &[String]) -> Result<(), ApplicationError> {
        Err(not_riichi())
    }
}

#[derive(Debug)]
pub(crate) struct GameRuntime {
    inner: Box<dyn RuleRuntime>,
}

impl GameRuntime {
    pub(crate) fn start(room: &Room, id: MatchId, now_ms: u64) -> Result<Self, ApplicationError> {
        match room.rule_snapshot() {
            GameRuleSnapshot::Riichi(_) => Ok(Self {
                inner: Box::new(RiichiRuntime::start(room, id, now_ms)?),
            }),
            GameRuleSnapshot::Impact(_) => Ok(Self {
                inner: Box::new(ImpactRuntime::start(room, id, now_ms)?),
            }),
        }
    }

    pub(crate) fn as_riichi(&self) -> Option<&RiichiRuntime> {
        self.inner.as_riichi()
    }

    /// 这张桌子会不会出牌谱。冲击麻将本期不生成记录，归档要照这个跳过。
    pub(crate) fn generates_record(&self) -> bool {
        self.inner.generates_record()
    }

    pub(crate) fn version(&self) -> u64 {
        self.inner.version()
    }

    pub(crate) fn event_sequence(&self) -> u64 {
        self.inner.event_sequence()
    }

    /// 服务端广播时随便挑一个在座的人当 actor，用来复算各家的可见视图。
    pub(crate) fn any_player(&self) -> Option<UserId> {
        self.inner.any_player()
    }

    pub(crate) fn projection(&self, actor: &UserId) -> Result<MatchProjection, ApplicationError> {
        self.inner.projection(actor)
    }

    pub(crate) fn events_after(
        &self,
        actor: &UserId,
        after_sequence: u64,
    ) -> Result<MatchEventPage, ApplicationError> {
        self.inner.events_after(actor, after_sequence)
    }

    pub(crate) fn execute(
        &mut self,
        actor: &UserId,
        command: SubmitGameCommand,
        now_ms: u64,
    ) -> Result<(), ApplicationError> {
        self.inner.execute(actor, command, now_ms)
    }

    pub(crate) fn set_dev_hand(
        &mut self,
        actor: &UserId,
        codes: &[String],
    ) -> Result<(), ApplicationError> {
        self.inner.set_dev_hand(actor, codes)
    }

    pub(crate) fn is_finished(&self) -> bool {
        self.inner.is_finished()
    }

    pub(crate) fn has_pending_settlement(&self) -> bool {
        self.inner.has_pending_settlement()
    }

    pub(crate) fn terminate_if_assets_stalled(
        &mut self,
        now_ms: u64,
    ) -> Result<bool, ApplicationError> {
        self.inner.terminate_if_assets_stalled(now_ms)
    }

    pub(crate) fn advance_settlement_if_due(
        &mut self,
        now_ms: u64,
    ) -> Result<bool, ApplicationError> {
        self.inner.advance_settlement_if_due(now_ms)
    }

    pub(crate) fn open_settlement_confirm_if_due(
        &mut self,
        now_ms: u64,
    ) -> Result<bool, ApplicationError> {
        self.inner.open_settlement_confirm_if_due(now_ms)
    }

    pub(crate) fn release_opening_if_due(&mut self, now_ms: u64) -> Result<bool, ApplicationError> {
        self.inner.release_opening_if_due(now_ms)
    }

    pub(crate) fn expire(&mut self, now_ms: u64) -> Result<Option<UserId>, ApplicationError> {
        self.inner.expire(now_ms)
    }
}

impl RuleRuntime for RiichiRuntime {
    fn as_riichi(&self) -> Option<&RiichiRuntime> {
        Some(self)
    }

    fn generates_record(&self) -> bool {
        true
    }

    fn version(&self) -> u64 {
        self.version
    }

    fn event_sequence(&self) -> u64 {
        self.event_sequence
    }

    fn any_player(&self) -> Option<UserId> {
        self.players.first().map(|player| player.user_id().clone())
    }

    fn projection(&self, actor: &UserId) -> Result<MatchProjection, ApplicationError> {
        Ok(MatchProjection::Riichi(Box::new(self.view(actor)?)))
    }

    fn events_after(
        &self,
        actor: &UserId,
        after_sequence: u64,
    ) -> Result<MatchEventPage, ApplicationError> {
        self.events_after(actor, after_sequence)
    }

    fn execute(
        &mut self,
        actor: &UserId,
        command: SubmitGameCommand,
        now_ms: u64,
    ) -> Result<(), ApplicationError> {
        self.execute(actor, command, now_ms)
    }

    fn is_finished(&self) -> bool {
        self.is_finished()
    }

    fn has_pending_settlement(&self) -> bool {
        self.has_pending_settlement()
    }

    fn terminate_if_assets_stalled(&mut self, now_ms: u64) -> Result<bool, ApplicationError> {
        self.terminate_if_assets_stalled(now_ms)
    }

    fn advance_settlement_if_due(&mut self, now_ms: u64) -> Result<bool, ApplicationError> {
        self.advance_settlement_if_due(now_ms)
    }

    fn open_settlement_confirm_if_due(&mut self, now_ms: u64) -> Result<bool, ApplicationError> {
        self.open_settlement_confirm_if_due(now_ms)
    }

    fn release_opening_if_due(&mut self, now_ms: u64) -> Result<bool, ApplicationError> {
        self.release_opening_if_due(now_ms)
    }

    fn expire(&mut self, now_ms: u64) -> Result<Option<UserId>, ApplicationError> {
        self.expire(now_ms)
    }

    fn set_dev_hand(&mut self, actor: &UserId, codes: &[String]) -> Result<(), ApplicationError> {
        self.set_dev_hand(actor, codes)
    }
}

impl RuleRuntime for ImpactRuntime {
    fn version(&self) -> u64 {
        self.version
    }

    fn event_sequence(&self) -> u64 {
        self.event_sequence
    }

    fn any_player(&self) -> Option<UserId> {
        self.players.first().map(|player| player.user_id().clone())
    }

    fn projection(&self, actor: &UserId) -> Result<MatchProjection, ApplicationError> {
        Ok(MatchProjection::Impact(Box::new(self.view(actor)?)))
    }

    fn events_after(
        &self,
        actor: &UserId,
        _after_sequence: u64,
    ) -> Result<MatchEventPage, ApplicationError> {
        self.seat_for(actor)?;
        Ok(MatchEventPage::new(
            self.version,
            self.event_sequence,
            Box::new([]),
        ))
    }

    fn execute(
        &mut self,
        actor: &UserId,
        command: SubmitGameCommand,
        now_ms: u64,
    ) -> Result<(), ApplicationError> {
        self.execute(actor, command, now_ms)
    }

    fn is_finished(&self) -> bool {
        self.is_finished()
    }

    fn has_pending_settlement(&self) -> bool {
        self.has_pending_settlement()
    }

    fn terminate_if_assets_stalled(&mut self, now_ms: u64) -> Result<bool, ApplicationError> {
        self.terminate_if_assets_stalled(now_ms)
    }

    fn advance_settlement_if_due(&mut self, now_ms: u64) -> Result<bool, ApplicationError> {
        self.advance_settlement_if_due(now_ms)
    }

    fn open_settlement_confirm_if_due(&mut self, now_ms: u64) -> Result<bool, ApplicationError> {
        self.open_settlement_confirm_if_due(now_ms)
    }

    fn release_opening_if_due(&mut self, now_ms: u64) -> Result<bool, ApplicationError> {
        self.release_opening_if_due(now_ms)
    }

    fn expire(&mut self, now_ms: u64) -> Result<Option<UserId>, ApplicationError> {
        self.expire(now_ms)
    }

    fn set_dev_hand(&mut self, actor: &UserId, codes: &[String]) -> Result<(), ApplicationError> {
        self.set_dev_hand(actor, codes)
    }
}

pub(crate) fn not_riichi() -> ApplicationError {
    ApplicationError::new(
        ErrorCode::InvalidGameCommand,
        "this match is not played with riichi rules",
    )
}
