use mahjong_riichi::{
    EndReason, HandResult, MatchEndReason, MatchResult, RiichiRuleSnapshot, Wind,
};
use serde::Serialize;

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
}

impl HandRecord {
    fn new(hand_index: u32, hand: &HandResult, events: &[GameEventRecord]) -> Self {
        let mut sequences = events
            .iter()
            .filter(|event| event.hand_index() == hand_index)
            .map(GameEventRecord::sequence);
        let first_event_sequence = sequences.next();
        let last_event_sequence = sequences.next_back().or(first_event_sequence);
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
