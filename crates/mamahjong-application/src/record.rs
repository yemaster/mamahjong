use mahjong_riichi::{
    DrawSource, EndReason, HandEvent, HandResult, MatchEndReason, MatchResult, Meld, MeldKind,
    ReactionKind, RiichiRuleSnapshot, Tile, WinSource, Wind,
};
use serde::Serialize;
use serde_json::{Value, json};

use crate::game::GameRuntime;
use crate::{ApplicationError, GameEventRecord};

#[derive(Clone, Debug, Serialize)]
pub struct MatchRecord {
    schema: &'static str,
    match_id: String,
    version: u64,
    event_sequence: u64,
    rule_snapshot: RiichiRuleSnapshot,
    players: Vec<RecordPlayer>,
    hands: Vec<HandRecord>,
    result: Option<FinalMatchRecord>,
}

impl MatchRecord {
    pub(crate) fn from_runtime(
        runtime: &GameRuntime,
        actor: &mahjong_core::UserId,
    ) -> Result<Self, ApplicationError> {
        runtime.seat_for(actor)?;
        Ok(Self {
            schema: "match_record.v1",
            match_id: runtime.id.as_str().to_owned(),
            version: runtime.version,
            event_sequence: runtime.event_sequence,
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
    from: Option<u8>,
    tenpai: Vec<u8>,
    nagashi_winners: Vec<u8>,
    awarded_riichi_sticks: u32,
    dealer_continues: bool,
    first_event_sequence: Option<u64>,
    last_event_sequence: Option<u64>,
    events: Vec<RecordEvent>,
}

impl HandRecord {
    fn new(hand_index: u32, hand: &HandResult, events: &[GameEventRecord]) -> Self {
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
            events: hand_events.into_iter().map(RecordEvent::from).collect(),
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
fn event_payload(event: &HandEvent) -> (&'static str, Value) {
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

const fn draw_source_name(source: DrawSource) -> &'static str {
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

const fn wind_name(value: Wind) -> &'static str {
    match value {
        Wind::East => "east",
        Wind::South => "south",
        Wind::West => "west",
        Wind::North => "north",
    }
}

const fn end_reason_name(value: EndReason) -> &'static str {
    match value {
        EndReason::ExhaustiveDraw => "exhaustive_draw",
        EndReason::NineTerminals => "nine_terminals",
        EndReason::FourWinds => "four_winds",
        EndReason::FourKans => "four_kans",
        EndReason::FourRiichi => "four_riichi",
        EndReason::Tsumo => "tsumo",
        EndReason::Ron => "ron",
    }
}
