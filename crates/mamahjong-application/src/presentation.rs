//! 一步操作在客户端上要播多久的动画。
//!
//! 前端 `apps/game-web/src/game/animationTiming.ts` 里有一份一模一样的表，常量名
//! 是对齐的。**改这里的数值必须同步改那边**。
//!
//! 服务端用这张表推迟下一个决策者的读秒：吃碰杠的牌还在往副露位上推、打出的牌
//! 还在往牌河飞的时候，谁都还看不见新的局面，这段时间不该算进任何人的思考时间。
//! 时钟因此按 `now + grace` 上弦，客户端拿到的读秒里也含着这段动画时间（见
//! `apps/server/src/clock.rs` 的 `SeatCountdown::snapshot`），两边始终对得上。

use crate::game::GameCommand;

/// 吃/碰/杠/立直 横幅从弹出到淡出的整段时间。
pub const CALL_BANNER_MS: u64 = 1_300;

/// 副露从手牌边缘推到副露位。
pub const MELD_PUSH_MS: u64 = 320;

/// 打出的牌从手里飞到牌河。
pub const DISCARD_FLIGHT_MS: u64 = 400;

/// 动画收尾到下家开始读秒之间留的一点白。
pub const ACTION_SETTLE_PADDING_MS: u64 = 120;

/// 结算摊牌阶段的硬上界：摊手、翻役种、流局逐家亮牌都在这段里。
///
/// 役种条目再多，客户端也必须在这个时刻之前进入结算读秒。
/// 这个值是用于客户端动画同步的上界；服务端兜底时限现在按役种条数动态计算，
/// 不再用这一个常数（见 `settlement_reveal_fallback_ms`）。
pub const SETTLEMENT_REVEAL_BUDGET_MS: u64 = 12_000;

/// 结算面板停在役种上、进入点数动画之前的停留时间。
pub const SETTLEMENT_COUNTDOWN_MS: u64 = 5_000;

/// 点棒增减演出的整段时长：数字淡入、停一拍看清、浮到分数上、分数滚到位。
pub const POINTS_REVEAL_MS: u64 = 2_800;

/// 确认窗口的倒计时：服务端开窗之后各家一起读这个秒。
///
/// 这段读秒的起点由服务端定、剩余时间由服务端下发，所有人的按钮和数字因此
/// 完全同步。倒计时走完服务端自己开下一局，不必等谁点。
pub const SETTLEMENT_CONFIRM_MS: u64 = 5_000;

/// 服务端兜底相对客户端播放上界留的余量。
pub const SETTLEMENT_FALLBACK_MARGIN_MS: u64 = 3_000;

/// 每条役种在结算摊牌阶段额外增加的兜底时长（毫秒）。
///
/// 前端逐条亮出役种，每条之间隔 250ms，加上初始延迟和收尾白；这里取 600ms
/// 给足 CSS 过渡和网络抖动的余量。
pub const PER_YAKU_REVEAL_MS: u64 = 600;

/// 一局都没人报告开局动画播完时，服务端最多等这么久就开打。
///
/// 只有完全不参与这个握手的客户端（例如控制台客户端）才会走到这里。
pub const OPENING_READY_FALLBACK_MS: u64 = 20_000;

/// 第一家报告动画播完之后，最多再等其他家这么久。
///
/// 开局摸牌和本局结算共用这一段宽限：两者的动画都由同一个事件触发、同时起播、
/// 时长只取决于视图数据，所以各家之间正常只差网络抖动。差得多的一般是被浏览器
/// 挂到后台节流了的页面，等下去没有意义，到期就当全场都播完了。
pub const ANIMATION_REPORT_GRACE_MS: u64 = 3_000;

/// 开局前等各家把对局音乐、素材load完的上限，超时就判定有人掉线并终止对局。
///
/// 这段不是动画，是网络：一首曲子几百 KB，慢的网要好几十秒，所以给得比别的宽限
/// 都长。真等不到的那家八成已经断了，让另外三家干等下去没有意义。
pub const MATCH_ASSET_LOAD_TIMEOUT_MS: u64 = 60_000;

const fn longer(left: u64, right: u64) -> u64 {
    if left > right { left } else { right }
}

/// 吃/碰/明杠/暗杠：横幅和推牌同时播，取长的那个。
#[must_use]
pub const fn meld_call_animation_ms() -> u64 {
    longer(CALL_BANNER_MS, MELD_PUSH_MS) + ACTION_SETTLE_PADDING_MS
}

/// 加杠只是往已经摆好的碰上再叠一张，没有推牌动画，只等横幅。
#[must_use]
pub const fn added_kan_animation_ms() -> u64 {
    CALL_BANNER_MS + ACTION_SETTLE_PADDING_MS
}

/// 普通打牌：牌飞到牌河为止。
#[must_use]
pub const fn discard_animation_ms() -> u64 {
    DISCARD_FLIGHT_MS + ACTION_SETTLE_PADDING_MS
}

/// 立直宣言的那张牌：横幅和飞牌同时播。
#[must_use]
pub const fn riichi_discard_animation_ms() -> u64 {
    longer(CALL_BANNER_MS, DISCARD_FLIGHT_MS) + ACTION_SETTLE_PADDING_MS
}

/// 一家都没报告结算动画播完时，服务端最多等这么久就开确认窗口。
///
/// 番种条数越多，前端逐条播报的时间越长；兜底时限在原有上界基础上按条数追加，
/// 役种再多也不会被服务端从中间切断。
#[must_use]
pub const fn settlement_reveal_fallback_ms(yaku_count: usize) -> u64 {
    SETTLEMENT_REVEAL_BUDGET_MS
        + SETTLEMENT_COUNTDOWN_MS
        + POINTS_REVEAL_MS
        + SETTLEMENT_FALLBACK_MARGIN_MS
        + (yaku_count as u64).saturating_mul(PER_YAKU_REVEAL_MS)
}

/// 全场既不上报也不点确认时，服务端替他们开下一局的时刻，从本局结算挂起算起。
#[must_use]
pub const fn settlement_fallback_ms(yaku_count: usize) -> u64 {
    settlement_reveal_fallback_ms(yaku_count) + SETTLEMENT_CONFIRM_MS
}

/// 这一步操作播完动画要多久，也就是下一个决策者的读秒该推迟多久。
///
/// 和了、流局和九种九牌走的是结算流程，结算自己有确认按钮和兜底计时，不占用
/// 座位时钟；跳过、准备、投票之类的操作桌面上没有任何动静，一律为零。
#[must_use]
pub fn animation_grace_ms(command: &GameCommand) -> u64 {
    match command {
        GameCommand::Discard { .. }
        | GameCommand::ImpactDiscard { .. }
        | GameCommand::SichuanDiscard { .. } => discard_animation_ms(),
        GameCommand::RiichiDiscard { .. } => riichi_discard_animation_ms(),
        GameCommand::Chi { .. }
        | GameCommand::Pon { .. }
        | GameCommand::OpenKan { .. }
        | GameCommand::ConcealedKan { .. }
        | GameCommand::Nuki { .. }
        | GameCommand::ImpactChi { .. }
        | GameCommand::ImpactPon
        | GameCommand::ImpactOpenKan
        | GameCommand::ImpactConcealedKan { .. }
        | GameCommand::ImpactIndicatorConcealedKan
        | GameCommand::SichuanPon
        | GameCommand::SichuanOpenKan
        | GameCommand::SichuanConcealedKan { .. } => meld_call_animation_ms(),
        GameCommand::AddedKan { .. }
        | GameCommand::ImpactAddedKan { .. }
        | GameCommand::SichuanAddedKan { .. } => added_kan_animation_ms(),
        GameCommand::Pass
        | GameCommand::Tsumo
        | GameCommand::Ron
        | GameCommand::NineTerminals
        | GameCommand::ImpactTsumo
        | GameCommand::ImpactRon
        | GameCommand::ImpactPass
        | GameCommand::ImpactKanAnimationPlayed { .. }
        | GameCommand::SichuanTsumo
        | GameCommand::SichuanRon
        | GameCommand::SichuanPass
        | GameCommand::SichuanExchange { .. }
        | GameCommand::SichuanDingQue { .. }
        | GameCommand::SichuanExchangeAnimationPlayed
        | GameCommand::SichuanWinAnimationPlayed { .. }
        | GameCommand::SichuanKanAnimationPlayed { .. }
        | GameCommand::MatchAssetsReady
        | GameCommand::ReadyForHand { .. }
        | GameCommand::SettlementPlayed { .. }
        | GameCommand::ConfirmSettlement { .. }
        | GameCommand::RequestExitVote
        | GameCommand::VoteExit { .. } => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ACTION_SETTLE_PADDING_MS, CALL_BANNER_MS, DISCARD_FLIGHT_MS, GameCommand, MELD_PUSH_MS,
        POINTS_REVEAL_MS, SETTLEMENT_CONFIRM_MS, SETTLEMENT_COUNTDOWN_MS,
        SETTLEMENT_REVEAL_BUDGET_MS, animation_grace_ms, settlement_fallback_ms,
        settlement_reveal_fallback_ms,
    };

    #[test]
    fn a_meld_waits_for_the_banner_because_it_outlasts_the_push() {
        const { assert!(MELD_PUSH_MS < CALL_BANNER_MS) };
        assert_eq!(
            animation_grace_ms(&GameCommand::Chi { tile_ids: [1, 2] }),
            CALL_BANNER_MS + ACTION_SETTLE_PADDING_MS
        );
        assert_eq!(
            animation_grace_ms(&GameCommand::Pon { tile_ids: [1, 2] }),
            animation_grace_ms(&GameCommand::Chi { tile_ids: [1, 2] })
        );
        assert_eq!(
            animation_grace_ms(&GameCommand::OpenKan {
                tile_ids: [1, 2, 3]
            }),
            animation_grace_ms(&GameCommand::Chi { tile_ids: [1, 2] })
        );
    }

    #[test]
    fn an_added_kan_only_waits_for_its_banner() {
        assert_eq!(
            animation_grace_ms(&GameCommand::AddedKan {
                meld_id: 1,
                tile_id: 2,
            }),
            CALL_BANNER_MS + ACTION_SETTLE_PADDING_MS
        );
    }

    #[test]
    fn a_discard_only_waits_for_the_tile_to_land() {
        assert_eq!(
            animation_grace_ms(&GameCommand::Discard { tile_id: 1 }),
            DISCARD_FLIGHT_MS + ACTION_SETTLE_PADDING_MS
        );
        assert!(
            animation_grace_ms(&GameCommand::Discard { tile_id: 1 })
                < animation_grace_ms(&GameCommand::RiichiDiscard { tile_id: 1 }),
            "立直宣言还要多播一条横幅"
        );
    }

    #[test]
    fn the_settlement_fallback_outlasts_everything_the_client_plays() {
        // 客户端从结算挂起到自己上报播完，最长就是这三段相加。
        let client_worst_case =
            SETTLEMENT_REVEAL_BUDGET_MS + SETTLEMENT_COUNTDOWN_MS + POINTS_REVEAL_MS;

        // 0 条役种：等于原先的固定上界，严格晚于客户端最差情况。
        assert!(
            settlement_reveal_fallback_ms(0) > client_worst_case,
            "服务端不能在客户端播完之前抢先开确认窗口"
        );
        // 役种越多，追加的兜底越多。
        assert!(
            settlement_reveal_fallback_ms(20) > settlement_reveal_fallback_ms(0),
            "20条役种的兜底应该比0条的更长"
        );
        assert!(
            settlement_reveal_fallback_ms(5) > settlement_reveal_fallback_ms(2),
            "5条役种的兜底应该比2条的更长"
        );
        assert_eq!(
            settlement_fallback_ms(3),
            settlement_reveal_fallback_ms(3) + SETTLEMENT_CONFIRM_MS,
            "开窗之后还要给满一整段确认倒计时"
        );
    }

    /// 从客户端那份镜像里取出一个常量的数值。
    fn client_const(source: &str, name: &str) -> u64 {
        let needle = format!("export const {name} = ");
        let rest = source
            .split_once(&needle)
            .unwrap_or_else(|| panic!("animationTiming.ts 里找不到常量 {name}"))
            .1;
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        digits
            .parse()
            .unwrap_or_else(|_| panic!("常量 {name} 的数值读不出来"))
    }

    /// 两份表一旦对不上，后端就会在前端还在播动画的时候扣下一家的思考时间，
    /// 或者在客户端播完结算之前抢先开下一局。约定见 docs/match-progression.md
    /// 第五节。
    #[test]
    fn the_client_animation_table_matches_this_one() {
        let source = include_str!("../../../apps/game-web/src/game/animationTiming.ts");

        for (name, ours) in [
            ("CALL_BANNER_MS", CALL_BANNER_MS),
            ("MELD_PUSH_MS", MELD_PUSH_MS),
            ("DISCARD_FLIGHT_MS", DISCARD_FLIGHT_MS),
            ("ACTION_SETTLE_PADDING_MS", ACTION_SETTLE_PADDING_MS),
            ("SETTLEMENT_REVEAL_BUDGET_MS", SETTLEMENT_REVEAL_BUDGET_MS),
            ("SETTLEMENT_COUNTDOWN_MS", SETTLEMENT_COUNTDOWN_MS),
            ("POINTS_REVEAL_MS", POINTS_REVEAL_MS),
            ("SETTLEMENT_CONFIRM_MS", SETTLEMENT_CONFIRM_MS),
        ] {
            assert_eq!(client_const(source, name), ours, "{name} 在前后端对不上");
        }
    }

    #[test]
    fn actions_without_an_animation_grant_no_grace() {
        assert_eq!(animation_grace_ms(&GameCommand::Pass), 0);
        assert_eq!(animation_grace_ms(&GameCommand::Tsumo), 0);
        assert_eq!(animation_grace_ms(&GameCommand::Ron), 0);
        assert_eq!(animation_grace_ms(&GameCommand::NineTerminals), 0);
    }
}
