use mahjong_riichi::{
    DrawSource, HandEvent, HandResult, MatchEndReason, MatchResult, Meld, MeldKind, ReactionKind,
    RiichiRuleSnapshot, Tile, WinSource,
};
use serde::Serialize;
use serde_json::{Value, json};

use crate::game::{HandWall, RiichiRuntime};
use crate::naming::{end_reason_name, limit_name, wind_name, yaku_name};
use crate::{ApplicationError, GameEventRecord};

#[derive(Clone, Debug, Serialize)]
pub struct MatchRecord {
    schema: &'static str,
    match_id: String,
    version: u64,
    event_sequence: u64,
    /// 好友对战还是段位匹配，牌谱列表的标题要用。
    friend_match: bool,
    rule_snapshot: RiichiRuleSnapshot,
    players: Vec<RecordPlayer>,
    hands: Vec<HandRecord>,
    result: Option<FinalMatchRecord>,
}

impl MatchRecord {
    pub(crate) fn from_runtime(
        runtime: &RiichiRuntime,
        actor: &mahjong_core::UserId,
    ) -> Result<Self, ApplicationError> {
        runtime.seat_for(actor)?;
        // 牌山只在对局结束之后才随牌谱下发：还在打的时候把牌山发出去，等于给客户端
        // 一份作弊器。这一条没有例外，也不许改成「只发已经打完的那几局」。
        let finished = runtime.game.result().is_some();
        Ok(Self {
            schema: "match_record.v1",
            match_id: runtime.id.as_str().to_owned(),
            version: runtime.version,
            event_sequence: runtime.event_sequence,
            friend_match: runtime.friend_match,
            rule_snapshot: runtime.rule_snapshot.clone(),
            players: runtime
                .players
                .iter()
                .map(|player| RecordPlayer {
                    user_id: player.user_id().as_str().to_owned(),
                    seat: player.seat().index(),
                    nickname: player.nickname().to_owned(),
                })
                .collect(),
            hands: runtime
                .game
                .hands()
                .iter()
                .enumerate()
                .map(|(index, hand)| {
                    HandRecord::new(
                        u32::try_from(index).expect("hand count is bounded by u32"),
                        hand,
                        &runtime.events,
                        finished.then(|| runtime.hand_walls.get(index)).flatten(),
                        runtime
                            .hand_ura_dora
                            .get(index)
                            .map_or(&[][..], |tiles| &tiles[..]),
                        runtime.game.rules().variant.seat_count().value(),
                    )
                })
                .collect(),
            result: runtime.game.result().map(FinalMatchRecord::from),
        })
    }

    #[must_use]
    pub fn match_id(&self) -> &str {
        &self.match_id
    }

    #[must_use]
    pub const fn version(&self) -> u64 {
        self.version
    }

    #[must_use]
    pub fn hand_count(&self) -> usize {
        self.hands.len()
    }

    #[must_use]
    pub const fn is_finished(&self) -> bool {
        self.result.is_some()
    }
}

#[derive(Clone, Debug, Serialize)]
struct RecordPlayer {
    user_id: String,
    seat: u8,
    nickname: String,
}

#[derive(Clone, Debug, Serialize)]
struct HandRecord {
    hand_index: u32,
    round_wind: &'static str,
    round_number: u8,
    dealer: u8,
    honba: u32,
    riichi_sticks: u32,
    reason: &'static str,
    points_before: Vec<i32>,
    point_deltas: Vec<i32>,
    points_after: Vec<i32>,
    winners: Vec<u8>,
    /// 和牌那几家的番符与役种，重演要靠它画结算面板。流局时是空的。
    winner_scores: Vec<WinnerScoreRecord>,
    /// 本局的里宝牌指示牌，没人和牌就是空的（流局不翻里宝牌）。
    ura_dora_indicators: Vec<Value>,
    from: Option<u8>,
    tenpai: Vec<u8>,
    nagashi_winners: Vec<u8>,
    awarded_riichi_sticks: u32,
    dealer_continues: bool,
    first_event_sequence: Option<u64>,
    last_event_sequence: Option<u64>,
    /// 本局洗好的牌山，对局结束之前一律为 `None`。
    wall: Option<WallSnapshot>,
    events: Vec<RecordEvent>,
}

impl HandRecord {
    fn new(
        hand_index: u32,
        hand: &HandResult,
        events: &[GameEventRecord],
        wall: Option<&HandWall>,
        ura_dora_indicators: &[Tile],
        seat_count: u8,
    ) -> Self {
        let hand_events: Vec<_> = events
            .iter()
            .filter(|event| event.hand_index() == hand_index)
            .collect();
        let first_event_sequence = hand_events.first().map(|event| event.sequence());
        let last_event_sequence = hand_events.last().map(|event| event.sequence());
        let progress = hand.progress();
        Self {
            hand_index,
            round_wind: wind_name(progress.round_wind()),
            round_number: progress.round_number().value(),
            dealer: progress.dealer().index(),
            honba: progress.honba().value(),
            riichi_sticks: progress.riichi_sticks().value(),
            reason: end_reason_name(hand.reason()),
            points_before: hand.points_before().to_vec(),
            point_deltas: hand.point_deltas().to_vec(),
            points_after: hand.points_after().to_vec(),
            winners: hand
                .winners()
                .iter()
                .map(|winner| winner.seat().index())
                .collect(),
            winner_scores: hand
                .winners()
                .iter()
                .map(|winner| {
                    WinnerScoreRecord::new(winner, winner.seat() == progress.dealer(), seat_count)
                })
                .collect(),
            ura_dora_indicators: ura_dora_indicators
                .iter()
                .copied()
                .map(tile_value)
                .collect(),
            from: hand.from().map(mahjong_riichi::Seat::index),
            tenpai: hand.tenpai().iter().map(|seat| seat.index()).collect(),
            nagashi_winners: hand
                .nagashi_winners()
                .iter()
                .map(|seat| seat.index())
                .collect(),
            awarded_riichi_sticks: hand.awarded_riichi_sticks(),
            dealer_continues: hand.dealer_continues(),
            first_event_sequence,
            last_event_sequence,
            wall: wall.map(WallSnapshot::from),
            events: hand_events.into_iter().map(RecordEvent::from).collect(),
        }
    }
}

/// 一家和牌的番符明细。
///
/// 字段和实时对局那份 `WinnerSettlementResponse`（`apps/server/src/api/dto.rs`）一一对应：
/// 重演直接把结算面板那套组件拿过来用，两边的形状必须一样。
#[derive(Clone, Debug, Serialize)]
struct WinnerScoreRecord {
    seat: u8,
    han: u8,
    fu: u16,
    yakuman_multiplier: u8,
    limit: &'static str,
    /// 这家实收的点数，本场棒和立直棒都算进去了。
    points: u32,
    dealer: bool,
    yaku: Vec<YakuRecord>,
}

impl WinnerScoreRecord {
    fn new(winner: &mahjong_riichi::ScoredWinner, dealer: bool, seat_count: u8) -> Self {
        let evaluation = winner.evaluation();
        let mut yaku: Vec<_> = evaluation
            .yaku()
            .iter()
            .map(|value| YakuRecord {
                name: yaku_name(value.yaku()),
                value: value.value(),
                yakuman: value.is_yakuman(),
            })
            .collect();
        // 役满不看宝牌，只有非役满的和牌才把宝牌／里宝／赤宝当成三行「役」补上去。
        if evaluation.yakuman_multiplier() == 0 {
            let bonuses = evaluation.bonuses();
            for (name, value) in [
                ("宝牌", bonuses.dora()),
                ("里宝牌", bonuses.ura_dora()),
                ("赤宝牌", bonuses.red_dora()),
                ("拔北宝牌", bonuses.nuki_dora()),
            ] {
                if value > 0 {
                    yaku.push(YakuRecord {
                        name,
                        value,
                        yakuman: false,
                    });
                }
            }
        }
        Self {
            seat: winner.seat().index(),
            han: evaluation.han(),
            fu: evaluation.fu(),
            yakuman_multiplier: evaluation.yakuman_multiplier(),
            limit: limit_name(evaluation.limit()),
            points: evaluation.payment().total_received(seat_count, dealer),
            dealer,
            yaku,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
struct YakuRecord {
    name: &'static str,
    value: u8,
    yakuman: bool,
}

/// 一局的完整牌山顺序。
///
/// `tiles[..live_end]` 是活牌区、按摸牌先后排，之后的十四张是王牌。重演要靠它画出
/// 没人摸到的那些牌；摸牌本身不会改动这个顺序，所以一局打完之后它仍然是完整的。
#[derive(Clone, Debug, Serialize)]
struct WallSnapshot {
    tiles: Vec<Value>,
    live_end: usize,
}

impl From<&HandWall> for WallSnapshot {
    fn from(value: &HandWall) -> Self {
        Self {
            tiles: value.tiles.iter().copied().map(tile_value).collect(),
            live_end: value.live_end,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
struct RecordEvent {
    sequence: u64,
    name: &'static str,
    event_version: u8,
    payload: Value,
}

impl From<&GameEventRecord> for RecordEvent {
    fn from(value: &GameEventRecord) -> Self {
        let (name, payload) = event_payload(value.event());
        Self {
            sequence: value.sequence(),
            name,
            event_version: 1,
            payload,
        }
    }
}

#[allow(clippy::too_many_lines)]
pub(crate) fn event_payload(event: &HandEvent) -> (&'static str, Value) {
    match event {
        HandEvent::HandStarted {
            progress,
            dora_indicator,
            remaining_live_draws,
        } => (
            "riichi.hand_started",
            json!({
                "round_wind": wind_name(progress.round_wind()),
                "round_number": progress.round_number().value(),
                "dealer": progress.dealer().index(),
                "honba": progress.honba().value(),
                "riichi_sticks": progress.riichi_sticks().value(),
                "dora_indicator": tile_value(*dora_indicator),
                "remaining_live_draws": remaining_live_draws,
            }),
        ),
        HandEvent::InitialHandDealt { seat, tiles } => (
            "riichi.initial_hand_dealt",
            json!({
                "seat": seat.index(),
                "tiles": tiles.iter().copied().map(tile_value).collect::<Vec<_>>(),
            }),
        ),
        HandEvent::TileDrawn {
            seat,
            tile,
            source,
            remaining_live_draws,
        } => (
            "riichi.tile_drawn",
            json!({
                "seat": seat.index(),
                "tile": tile_value(*tile),
                "source": draw_source_name(*source),
                "remaining_live_draws": remaining_live_draws,
            }),
        ),
        HandEvent::TileDiscarded {
            seat,
            tile,
            tsumogiri,
            riichi_declared,
        } => (
            "riichi.tile_discarded",
            json!({
                "seat": seat.index(),
                "tile": tile_value(*tile),
                "tsumogiri": tsumogiri,
                "riichi_declared": riichi_declared,
            }),
        ),
        HandEvent::NorthExtracted { seat, tile } => (
            "riichi.north_extracted",
            json!({"seat": seat.index(), "tile": tile_value(*tile)}),
        ),
        HandEvent::ReactionSubmitted { seat, reaction } => (
            "riichi.reaction_submitted",
            json!({
                "seat": seat.index(),
                "reaction": reaction_name(*reaction),
            }),
        ),
        HandEvent::MeldDeclared { seat, meld } => (
            "riichi.meld_declared",
            json!({"seat": seat.index(), "meld": meld_value(meld)}),
        ),
        HandEvent::KanProposed {
            seat,
            kind,
            tile_kind,
        } => (
            "riichi.kan_proposed",
            json!({
                "seat": seat.index(),
                "kind": meld_kind_name(*kind),
                "tile_kind": tile_kind.to_string(),
            }),
        ),
        HandEvent::KanCompleted { seat, meld } => (
            "riichi.kan_completed",
            json!({"seat": seat.index(), "meld": meld_value(meld)}),
        ),
        HandEvent::DoraIndicatorRevealed {
            tile,
            revealed_count,
        } => (
            "riichi.dora_indicator_revealed",
            json!({
                "tile": tile_value(*tile),
                "revealed_count": revealed_count,
            }),
        ),
        HandEvent::RiichiEstablished {
            seat,
            points_after,
            riichi_sticks,
        } => (
            "riichi.riichi_established",
            json!({
                "seat": seat.index(),
                "points_after": points_after,
                "riichi_sticks": riichi_sticks,
            }),
        ),
        HandEvent::IppatsuExpired { seat } => {
            ("riichi.ippatsu_expired", json!({"seat": seat.index()}))
        }
        HandEvent::IppatsuCancelled { seats } => (
            "riichi.ippatsu_cancelled",
            json!({"seats": seats.iter().map(|seat| seat.index()).collect::<Vec<_>>()}),
        ),
        HandEvent::FuritenChanged {
            seat,
            temporary,
            riichi,
        } => (
            "riichi.furiten_changed",
            json!({
                "seat": seat.index(),
                "temporary": temporary,
                "riichi": riichi,
            }),
        ),
        HandEvent::TsumoDeclared {
            winner,
            tile,
            source,
        } => (
            "riichi.tsumo_declared",
            json!({
                "winner": winner.index(),
                "tile": tile_value(*tile),
                "source": draw_source_name(*source),
            }),
        ),
        HandEvent::RonDeclared {
            winners,
            from,
            tile,
            source,
        } => (
            "riichi.ron_declared",
            json!({
                "winners": winners.iter().map(|seat| seat.index()).collect::<Vec<_>>(),
                "from": from.index(),
                "tile": tile_value(*tile),
                "source": win_source_value(*source),
            }),
        ),
        HandEvent::AbortiveDrawDeclared { reason, declarer } => (
            "riichi.abortive_draw_declared",
            json!({
                "reason": end_reason_name(*reason),
                "declarer": declarer.map(mahjong_riichi::Seat::index),
            }),
        ),
        HandEvent::ExhaustiveDrawDeclared { reason, tenpai } => (
            "riichi.exhaustive_draw_declared",
            json!({
                "reason": end_reason_name(*reason),
                "tenpai": tenpai.iter().map(|seat| seat.index()).collect::<Vec<_>>(),
            }),
        ),
    }
}

fn tile_value(tile: Tile) -> Value {
    json!({"id": tile.id().value(), "code": tile.to_string()})
}

fn meld_value(meld: &Meld) -> Value {
    json!({
        "id": meld.id().value(),
        "kind": meld_kind_name(meld.kind()),
        "tiles": meld.tiles().iter().copied().map(tile_value).collect::<Vec<_>>(),
        "called_from": meld.called_from().map(mahjong_riichi::Seat::index),
        "called_tile_id": meld.called_tile().map(mahjong_riichi::TileId::value),
    })
}

pub(crate) const fn draw_source_name(source: DrawSource) -> &'static str {
    match source {
        DrawSource::LiveWall => "live_wall",
        DrawSource::Rinshan => "rinshan",
    }
}

const fn reaction_name(reaction: ReactionKind) -> &'static str {
    match reaction {
        ReactionKind::Pass => "pass",
        ReactionKind::Ron => "ron",
        ReactionKind::Chi => "chi",
        ReactionKind::Pon => "pon",
        ReactionKind::OpenKan => "open_kan",
    }
}

const fn meld_kind_name(kind: MeldKind) -> &'static str {
    match kind {
        MeldKind::Chi => "chi",
        MeldKind::Pon => "pon",
        MeldKind::OpenKan => "open_kan",
        MeldKind::ConcealedKan => "concealed_kan",
        MeldKind::AddedKan => "added_kan",
    }
}

fn win_source_value(source: WinSource) -> Value {
    match source {
        WinSource::Tsumo(source) => {
            json!({"kind": "tsumo", "draw_source": draw_source_name(source)})
        }
        WinSource::Discard { from } => json!({"kind": "discard", "from": from.index()}),
        WinSource::AddedKan { from, meld_id } => {
            json!({"kind": "added_kan", "from": from.index(), "meld_id": meld_id.value()})
        }
        WinSource::ConcealedKan { from } => {
            json!({"kind": "concealed_kan", "from": from.index()})
        }
        WinSource::Nuki { from } => json!({"kind": "nuki", "from": from.index()}),
    }
}

#[derive(Clone, Debug, Serialize)]
struct FinalMatchRecord {
    end_reason: &'static str,
    hand_count: u32,
    final_points: Vec<i32>,
    placements: Vec<RecordPlacement>,
    unclaimed_riichi_sticks_awarded: u32,
}

impl From<&MatchResult> for FinalMatchRecord {
    fn from(value: &MatchResult) -> Self {
        Self {
            end_reason: match value.end_reason() {
                MatchEndReason::ScheduledEnd => "scheduled_end",
                MatchEndReason::Tobi => "tobi",
                MatchEndReason::AgariYame => "agari_yame",
            },
            hand_count: value.hand_count(),
            final_points: value.final_points().to_vec(),
            placements: value
                .placements()
                .iter()
                .map(|placement| RecordPlacement {
                    seat: placement.seat().index(),
                    rank: placement.rank(),
                    points: placement.points(),
                    uma_tenths: placement.uma_tenths(),
                    oka_tenths: placement.oka_tenths(),
                    score_tenths: placement.score_tenths(),
                })
                .collect(),
            unclaimed_riichi_sticks_awarded: value.unclaimed_riichi_sticks_awarded(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
struct RecordPlacement {
    seat: u8,
    rank: u8,
    points: i32,
    uma_tenths: i32,
    oka_tenths: i32,
    score_tenths: i32,
}
