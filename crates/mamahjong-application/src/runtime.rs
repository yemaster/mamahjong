//! 两套规则引擎共用的运行时外壳。
//!
//! 房间里坐的是哪套规则，在 `GameRuntime::start` 里定下来，之后所有会话级操作
//! （读秒、素材握手、结算放行、退出投票）都从这里分派。立直那条分支一行没改，
//! 只是被包了一层。

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

#[derive(Clone, Debug)]
pub(crate) enum GameRuntime {
    Riichi(Box<RiichiRuntime>),
    Impact(Box<ImpactRuntime>),
}

impl GameRuntime {
    pub(crate) fn start(room: &Room, id: MatchId, now_ms: u64) -> Result<Self, ApplicationError> {
        match room.rule_snapshot() {
            GameRuleSnapshot::Riichi(_) => Ok(Self::Riichi(Box::new(RiichiRuntime::start(
                room, id, now_ms,
            )?))),
            GameRuleSnapshot::Impact(_) => Ok(Self::Impact(Box::new(ImpactRuntime::start(
                room, id, now_ms,
            )?))),
        }
    }

    pub(crate) const fn as_riichi(&self) -> Option<&RiichiRuntime> {
        match self {
            Self::Riichi(runtime) => Some(runtime),
            Self::Impact(_) => None,
        }
    }

    /// 这张桌子会不会出牌谱。冲击麻将本期不生成记录，归档要照这个跳过。
    pub(crate) fn generates_record(&self) -> bool {
        matches!(self, Self::Riichi(_))
    }

    pub(crate) fn version(&self) -> u64 {
        match self {
            Self::Riichi(runtime) => runtime.version,
            Self::Impact(runtime) => runtime.version,
        }
    }

    pub(crate) fn event_sequence(&self) -> u64 {
        match self {
            Self::Riichi(runtime) => runtime.event_sequence,
            Self::Impact(runtime) => runtime.event_sequence,
        }
    }

    /// 服务端广播时随便挑一个在座的人当 actor，用来复算各家的可见视图。
    pub(crate) fn any_player(&self) -> Option<UserId> {
        match self {
            Self::Riichi(runtime) => runtime
                .players
                .first()
                .map(|player| player.user_id().clone()),
            Self::Impact(runtime) => runtime
                .players
                .first()
                .map(|player| player.user_id().clone()),
        }
    }

    pub(crate) fn projection(&self, actor: &UserId) -> Result<MatchProjection, ApplicationError> {
        match self {
            Self::Riichi(runtime) => Ok(MatchProjection::Riichi(Box::new(runtime.view(actor)?))),
            Self::Impact(runtime) => Ok(MatchProjection::Impact(Box::new(runtime.view(actor)?))),
        }
    }

    pub(crate) fn events_after(
        &self,
        actor: &UserId,
        after_sequence: u64,
    ) -> Result<MatchEventPage, ApplicationError> {
        match self {
            Self::Riichi(runtime) => runtime.events_after(actor, after_sequence),
            // 冲击麻将暂不生成事件流，客户端一律走视图订阅。
            Self::Impact(runtime) => {
                runtime.seat_for(actor)?;
                Ok(MatchEventPage::new(
                    runtime.version,
                    runtime.event_sequence,
                    Box::new([]),
                ))
            }
        }
    }

    pub(crate) fn execute(
        &mut self,
        actor: &UserId,
        command: SubmitGameCommand,
        now_ms: u64,
    ) -> Result<(), ApplicationError> {
        match self {
            Self::Riichi(runtime) => runtime.execute(actor, command, now_ms),
            Self::Impact(runtime) => runtime.execute(actor, command, now_ms),
        }
    }

    pub(crate) fn is_finished(&self) -> bool {
        match self {
            Self::Riichi(runtime) => runtime.is_finished(),
            Self::Impact(runtime) => runtime.is_finished(),
        }
    }

    pub(crate) fn has_pending_settlement(&self) -> bool {
        match self {
            Self::Riichi(runtime) => runtime.has_pending_settlement(),
            Self::Impact(runtime) => runtime.has_pending_settlement(),
        }
    }

    pub(crate) fn terminate_if_assets_stalled(
        &mut self,
        now_ms: u64,
    ) -> Result<bool, ApplicationError> {
        match self {
            Self::Riichi(runtime) => runtime.terminate_if_assets_stalled(now_ms),
            Self::Impact(runtime) => runtime.terminate_if_assets_stalled(now_ms),
        }
    }

    pub(crate) fn advance_settlement_if_due(
        &mut self,
        now_ms: u64,
    ) -> Result<bool, ApplicationError> {
        match self {
            Self::Riichi(runtime) => runtime.advance_settlement_if_due(now_ms),
            Self::Impact(runtime) => runtime.advance_settlement_if_due(now_ms),
        }
    }

    pub(crate) fn open_settlement_confirm_if_due(
        &mut self,
        now_ms: u64,
    ) -> Result<bool, ApplicationError> {
        match self {
            Self::Riichi(runtime) => runtime.open_settlement_confirm_if_due(now_ms),
            Self::Impact(runtime) => runtime.open_settlement_confirm_if_due(now_ms),
        }
    }

    pub(crate) fn release_opening_if_due(&mut self, now_ms: u64) -> Result<bool, ApplicationError> {
        match self {
            Self::Riichi(runtime) => runtime.release_opening_if_due(now_ms),
            Self::Impact(runtime) => runtime.release_opening_if_due(now_ms),
        }
    }

    pub(crate) fn expire(&mut self, now_ms: u64) -> Result<Option<UserId>, ApplicationError> {
        match self {
            Self::Riichi(runtime) => runtime.expire(now_ms),
            Self::Impact(runtime) => runtime.expire(now_ms),
        }
    }
}

pub(crate) fn not_riichi() -> ApplicationError {
    ApplicationError::new(
        ErrorCode::InvalidGameCommand,
        "this match is not played with riichi rules",
    )
}
