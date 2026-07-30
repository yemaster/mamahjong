use mahjong_core::{MatchId, UserId};
use mahjong_riichi::{
    Discard, HandEvent, HandOutcome, HandPhase, HandSettlement, MatchResult, Meld, MeldId,
    Reaction, RiichiHand, RiichiMatch, RiichiScorer, RiichiStatus, ScoredWinner, Seat,
    TableProgress, Tile, TileId, WallSeed, WinEvaluation,
};

use crate::{ApplicationError, ErrorCode, GameRuleSnapshot, Room, RoomLifecycle};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GameCommand {
    Discard { tile_id: u16 },
    RiichiDiscard { tile_id: u16 },
    Tsumo,
    Pass,
    Ron,
    Chi { tile_ids: [u16; 2] },
    Pon { tile_ids: [u16; 2] },
    OpenKan { tile_ids: [u16; 3] },
    ConcealedKan { tile_ids: [u16; 4] },
    AddedKan { meld_id: u8, tile_id: u16 },
    NineTerminals,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubmitGameCommand {
    pub expected_version: u64,
    pub command: GameCommand,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MatchPlayer {
    user_id: UserId,
    seat: Seat,
    nickname: String,
}

impl MatchPlayer {
    #[must_use]
    pub const fn user_id(&self) -> &UserId {
        &self.user_id
    }

    #[must_use]
    pub const fn seat(&self) -> Seat {
        self.seat
    }

    #[must_use]
    pub fn nickname(&self) -> &str {
        &self.nickname
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GameEventRecord {
    sequence: u64,
    hand_index: u32,
    event: HandEvent,
}

impl GameEventRecord {
    #[must_use]
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    #[must_use]
    pub const fn hand_index(&self) -> u32 {
        self.hand_index
    }

    #[must_use]
    pub const fn event(&self) -> &HandEvent {
        &self.event
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObserverPlayer {
    player: MatchPlayer,
    points: i32,
    concealed_tiles: Option<Box<[Tile]>>,
    concealed_tile_count: usize,
    drawn_tile_id: Option<TileId>,
    melds: Box<[Meld]>,
    discards: Box<[Discard]>,
    riichi_status: RiichiStatus,
}

impl ObserverPlayer {
    #[must_use]
    pub const fn player(&self) -> &MatchPlayer {
        &self.player
    }

    #[must_use]
    pub const fn points(&self) -> i32 {
        self.points
    }

    #[must_use]
    pub fn concealed_tiles(&self) -> Option<&[Tile]> {
        self.concealed_tiles.as_deref()
    }

    #[must_use]
    pub const fn concealed_tile_count(&self) -> usize {
        self.concealed_tile_count
    }

    #[must_use]
    pub const fn drawn_tile_id(&self) -> Option<TileId> {
        self.drawn_tile_id
    }

    #[must_use]
    pub fn melds(&self) -> &[Meld] {
        &self.melds
    }

    #[must_use]
    pub fn discards(&self) -> &[Discard] {
        &self.discards
    }

    #[must_use]
    pub const fn riichi_status(&self) -> RiichiStatus {
        self.riichi_status
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObserverMatch {
    id: MatchId,
    observer_seat: Seat,
    version: u64,
    event_sequence: u64,
    hand_index: u32,
    progress: TableProgress,
    phase: HandPhase,
    remaining_live_draws: usize,
    dora_indicators: Box<[Tile]>,
    players: Box<[ObserverPlayer]>,
    result: Option<MatchResult>,
}

impl ObserverMatch {
    #[must_use]
    pub const fn id(&self) -> &MatchId {
        &self.id
    }

    #[must_use]
    pub const fn observer_seat(&self) -> Seat {
        self.observer_seat
    }

    #[must_use]
    pub const fn version(&self) -> u64 {
        self.version
    }

    #[must_use]
    pub const fn event_sequence(&self) -> u64 {
        self.event_sequence
    }

    #[must_use]
    pub const fn hand_index(&self) -> u32 {
        self.hand_index
    }

    #[must_use]
    pub const fn progress(&self) -> TableProgress {
        self.progress
    }

    #[must_use]
    pub const fn phase(&self) -> HandPhase {
        self.phase
    }

    #[must_use]
    pub const fn remaining_live_draws(&self) -> usize {
        self.remaining_live_draws
    }

    #[must_use]
    pub fn dora_indicators(&self) -> &[Tile] {
        &self.dora_indicators
    }

    #[must_use]
    pub fn players(&self) -> &[ObserverPlayer] {
        &self.players
    }

    #[must_use]
    pub const fn result(&self) -> Option<&MatchResult> {
        self.result.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GameRuntime {
    id: MatchId,
    version: u64,
    event_sequence: u64,
    hand_index: u32,
    players: Box<[MatchPlayer]>,
    game: RiichiMatch,
    hand: RiichiHand,
    events: Vec<GameEventRecord>,
    ron_evaluations: Box<[Option<WinEvaluation>]>,
}

impl GameRuntime {
    pub(crate) fn start(room: &Room, id: MatchId) -> Result<Self, ApplicationError> {
        if room.lifecycle() != RoomLifecycle::Playing || room.active_match_id() != Some(&id) {
            return Err(internal_error("room is not linked to the starting match"));
        }
        let GameRuleSnapshot::Riichi(snapshot) = room.rule_snapshot();
        let rules = snapshot.rules().clone();
        let dealer = Seat::new(rules.variant, 0)
            .map_err(|_| internal_error("starting dealer is invalid"))?;
        let game = RiichiMatch::start(rules.clone(), dealer)
            .map_err(|error| internal_error(error.to_string()))?;
        let seed = WallSeed::generate().map_err(|error| internal_error(error.to_string()))?;
        let (hand, transition) =
            RiichiHand::start(rules, game.progress(), game.points().iter().copied(), &seed)
                .map_err(|error| internal_error(error.to_string()))?;
        let players = room
            .members()
            .iter()
            .map(|member| {
                Ok(MatchPlayer {
                    user_id: member.user_id().clone(),
                    seat: Seat::new(snapshot.rules().variant, member.seat())
                        .map_err(|_| internal_error("room member seat is invalid"))?,
                    nickname: member.nickname().to_owned(),
                })
            })
            .collect::<Result<Vec<_>, ApplicationError>>()?
            .into_boxed_slice();
        let seat_count = usize::from(snapshot.rules().variant.seat_count().value());
        let mut runtime = Self {
            id,
            version: 1,
            event_sequence: 0,
            hand_index: 0,
            players,
            game,
            hand,
            events: Vec::new(),
            ron_evaluations: vec![None; seat_count].into_boxed_slice(),
        };
        runtime.record_events(transition.into_events())?;
        Ok(runtime)
    }

    pub(crate) fn view(&self, actor: &UserId) -> Result<ObserverMatch, ApplicationError> {
        let actor_seat = self.seat_for(actor)?;
        let players = self
            .players
            .iter()
            .map(|match_player| {
                let hand = self
                    .hand
                    .player(match_player.seat)
                    .map_err(|error| internal_error(error.to_string()))?;
                let concealed_tiles = (match_player.seat == actor_seat)
                    .then(|| hand.concealed().to_vec().into_boxed_slice());
                Ok(ObserverPlayer {
                    player: match_player.clone(),
                    points: self.game.result().map_or(hand.points(), |result| {
                        result.final_points()[usize::from(match_player.seat.index())]
                    }),
                    concealed_tile_count: hand.concealed().len(),
                    concealed_tiles,
                    drawn_tile_id: (match_player.seat == actor_seat)
                        .then_some(hand.drawn_tile_id())
                        .flatten(),
                    melds: hand.melds().to_vec().into_boxed_slice(),
                    discards: hand.discards().to_vec().into_boxed_slice(),
                    riichi_status: hand.riichi_status(),
                })
            })
            .collect::<Result<Vec<_>, ApplicationError>>()?
            .into_boxed_slice();
        Ok(ObserverMatch {
            id: self.id.clone(),
            observer_seat: actor_seat,
            version: self.version,
            event_sequence: self.event_sequence,
            hand_index: self.hand_index,
            progress: self.hand.progress(),
            phase: self.hand.phase(),
            remaining_live_draws: self.hand.remaining_live_draws(),
            dora_indicators: self
                .hand
                .current_dora_indicators()
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            players,
            result: self.game.result().cloned(),
        })
    }

    pub(crate) fn execute(
        &mut self,
        actor: &UserId,
        command: SubmitGameCommand,
    ) -> Result<(), ApplicationError> {
        if self.game.result().is_some() {
            return Err(ApplicationError::new(
                ErrorCode::MatchFinished,
                "match is already finished",
            ));
        }
        if command.expected_version != self.version {
            return Err(ApplicationError::new(
                ErrorCode::MatchVersionConflict,
                format!(
                    "expected match version {}, current version is {}",
                    command.expected_version, self.version
                ),
            ));
        }
        let seat = self.seat_for(actor)?;
        let mut tsumo_evaluation = None;
        let transition = match command.command {
            GameCommand::Discard { tile_id } => self.hand.discard(seat, TileId::new(tile_id)),
            GameCommand::RiichiDiscard { tile_id } => {
                self.hand
                    .declare_riichi_and_discard(seat, TileId::new(tile_id), &RiichiScorer)
            }
            GameCommand::Tsumo => {
                tsumo_evaluation = Some(self.hand.evaluate_tsumo(seat).map_err(invalid_command)?);
                self.hand.declare_tsumo(seat, &RiichiScorer)
            }
            GameCommand::Pass => self.hand.pass(seat, &RiichiScorer),
            GameCommand::Ron => {
                let evaluation = self
                    .hand
                    .evaluate_pending_ron(seat)
                    .map_err(invalid_command)?;
                let transition = self.hand.respond(seat, Reaction::Ron, &RiichiScorer);
                if transition.is_ok() {
                    self.ron_evaluations[usize::from(seat.index())] = Some(evaluation);
                }
                transition
            }
            GameCommand::Chi { tile_ids } => self.hand.respond(
                seat,
                Reaction::Chi {
                    hand_tiles: tile_ids.map(TileId::new),
                },
                &RiichiScorer,
            ),
            GameCommand::Pon { tile_ids } => self.hand.respond(
                seat,
                Reaction::Pon {
                    hand_tiles: tile_ids.map(TileId::new),
                },
                &RiichiScorer,
            ),
            GameCommand::OpenKan { tile_ids } => self.hand.respond(
                seat,
                Reaction::OpenKan {
                    hand_tiles: tile_ids.map(TileId::new),
                },
                &RiichiScorer,
            ),
            GameCommand::ConcealedKan { tile_ids } => {
                self.hand
                    .declare_concealed_kan(seat, tile_ids.map(TileId::new), &RiichiScorer)
            }
            GameCommand::AddedKan { meld_id, tile_id } => {
                self.hand
                    .declare_added_kan(seat, MeldId::new(meld_id), TileId::new(tile_id))
            }
            GameCommand::NineTerminals => self.hand.declare_nine_terminals(seat),
        }
        .map_err(invalid_command)?;

        let events = transition.into_events();
        self.synchronize_riichi(&events)?;
        let ended = events.iter().find(|event| {
            matches!(
                event,
                HandEvent::TsumoDeclared { .. }
                    | HandEvent::RonDeclared { .. }
                    | HandEvent::AbortiveDrawDeclared { .. }
                    | HandEvent::ExhaustiveDrawDeclared { .. }
            )
        });
        let outcome = ended
            .map(|event| self.outcome_from_event(event, tsumo_evaluation))
            .transpose()?;
        self.record_events(events)?;
        if let Some(outcome) = outcome {
            self.finish_hand(outcome)?;
        }
        self.version = self
            .version
            .checked_add(1)
            .ok_or_else(|| internal_error("match version overflow"))?;
        Ok(())
    }

    fn synchronize_riichi(&mut self, events: &[HandEvent]) -> Result<(), ApplicationError> {
        for event in events {
            if let HandEvent::RiichiEstablished {
                seat,
                points_after,
                riichi_sticks,
            } = event
            {
                self.game
                    .record_riichi_established(*seat, *points_after, *riichi_sticks)
                    .map_err(|error| internal_error(error.to_string()))?;
            }
        }
        Ok(())
    }

    fn outcome_from_event(
        &self,
        event: &HandEvent,
        tsumo_evaluation: Option<WinEvaluation>,
    ) -> Result<HandOutcome, ApplicationError> {
        match event {
            HandEvent::TsumoDeclared { winner, .. } => Ok(HandOutcome::Tsumo {
                winner: ScoredWinner::new(
                    *winner,
                    tsumo_evaluation
                        .ok_or_else(|| internal_error("tsumo evaluation is missing"))?,
                ),
            }),
            HandEvent::RonDeclared { winners, from, .. } => Ok(HandOutcome::Ron {
                from: *from,
                winners: winners
                    .iter()
                    .map(|winner| {
                        self.ron_evaluations[usize::from(winner.index())]
                            .clone()
                            .map(|evaluation| ScoredWinner::new(*winner, evaluation))
                            .ok_or_else(|| internal_error("ron evaluation is missing"))
                    })
                    .collect::<Result<Vec<_>, _>>()?
                    .into_boxed_slice(),
            }),
            HandEvent::ExhaustiveDrawDeclared { tenpai, .. } => {
                let nagashi_winners = if self.game.rules().scoring.nagashi_mangan {
                    self.players
                        .iter()
                        .filter_map(|player| {
                            let hand = self.hand.player(player.seat).ok()?;
                            RiichiScorer.is_nagashi_mangan(hand).then_some(player.seat)
                        })
                        .collect::<Vec<_>>()
                        .into_boxed_slice()
                } else {
                    Box::new([])
                };
                Ok(HandOutcome::ExhaustiveDraw {
                    tenpai: if nagashi_winners.is_empty() {
                        tenpai.clone()
                    } else {
                        Box::new([])
                    },
                    nagashi_winners,
                })
            }
            HandEvent::AbortiveDrawDeclared { reason, .. } => {
                Ok(HandOutcome::AbortiveDraw { reason: *reason })
            }
            _ => Err(internal_error("event does not finish a hand")),
        }
    }

    fn finish_hand(&mut self, outcome: HandOutcome) -> Result<(), ApplicationError> {
        let result = HandSettlement
            .settle(
                self.game.rules(),
                self.game.progress(),
                self.game.points().iter().copied(),
                outcome,
            )
            .map_err(|error| internal_error(error.to_string()))?;
        self.game
            .apply_hand(result)
            .map_err(|error| internal_error(error.to_string()))?;
        self.ron_evaluations.fill(None);
        self.hand_index = u32::try_from(self.game.hands().len())
            .map_err(|_| internal_error("hand index overflow"))?;
        if self.game.result().is_some() {
            return Ok(());
        }
        let seed = WallSeed::generate().map_err(|error| internal_error(error.to_string()))?;
        let (hand, transition) = RiichiHand::start(
            self.game.rules().clone(),
            self.game.progress(),
            self.game.points().iter().copied(),
            &seed,
        )
        .map_err(|error| internal_error(error.to_string()))?;
        self.hand = hand;
        self.record_events(transition.into_events())
    }

    fn record_events(&mut self, events: Box<[HandEvent]>) -> Result<(), ApplicationError> {
        self.events.reserve(events.len());
        for event in events {
            self.event_sequence = self
                .event_sequence
                .checked_add(1)
                .ok_or_else(|| internal_error("event sequence overflow"))?;
            self.events.push(GameEventRecord {
                sequence: self.event_sequence,
                hand_index: self.hand_index,
                event,
            });
        }
        Ok(())
    }

    fn seat_for(&self, user_id: &UserId) -> Result<Seat, ApplicationError> {
        self.players
            .iter()
            .find(|player| &player.user_id == user_id)
            .map(|player| player.seat)
            .ok_or_else(|| {
                ApplicationError::new(
                    ErrorCode::NotMatchPlayer,
                    "user is not a player in this match",
                )
            })
    }
}

fn invalid_command(error: impl std::fmt::Display) -> ApplicationError {
    ApplicationError::new(ErrorCode::InvalidGameCommand, error.to_string())
}

fn internal_error(message: impl Into<String>) -> ApplicationError {
    ApplicationError::new(ErrorCode::Internal, message)
}
