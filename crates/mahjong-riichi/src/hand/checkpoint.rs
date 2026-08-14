use std::collections::HashSet;
use std::error::Error;
use std::fmt::{self, Debug, Display, Formatter};

use crate::{MeldKind, Reaction, RiichiHand, RiichiStatus, Seat, Tile, TileId, TileKind};

use super::state::{PendingKan, Phase};

pub const HAND_CHECKPOINT_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Eq, PartialEq)]
pub struct RiichiHandCheckpoint {
    schema_version: u16,
    last_event_sequence: u64,
    state: RiichiHand,
}

impl RiichiHandCheckpoint {
    #[must_use]
    pub const fn schema_version(&self) -> u16 {
        self.schema_version
    }

    #[must_use]
    pub const fn last_event_sequence(&self) -> u64 {
        self.last_event_sequence
    }

    pub fn restore(self) -> Result<(RiichiHand, u64), HandCheckpointError> {
        if self.schema_version != HAND_CHECKPOINT_SCHEMA_VERSION {
            return Err(HandCheckpointError::UnsupportedSchema {
                actual: self.schema_version,
                supported: HAND_CHECKPOINT_SCHEMA_VERSION,
            });
        }
        self.state
            .validate_invariants()
            .map_err(HandCheckpointError::InvalidState)?;
        Ok((self.state, self.last_event_sequence))
    }
}

impl Debug for RiichiHandCheckpoint {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RiichiHandCheckpoint")
            .field("schema_version", &self.schema_version)
            .field("last_event_sequence", &self.last_event_sequence)
            .field("variant", &self.state.rules.variant)
            .field("phase", &self.state.phase())
            .field("state", &"[REDACTED]")
            .finish()
    }
}

impl RiichiHand {
    pub fn checkpoint(
        &self,
        last_event_sequence: u64,
    ) -> Result<RiichiHandCheckpoint, HandInvariantError> {
        self.validate_invariants()?;
        Ok(RiichiHandCheckpoint {
            schema_version: HAND_CHECKPOINT_SCHEMA_VERSION,
            last_event_sequence,
            state: self.clone(),
        })
    }

    pub fn validate_invariants(&self) -> Result<(), HandInvariantError> {
        self.validate_structural_invariants()?;
        self.validate_tile_invariants()?;
        self.validate_phase_invariants()
    }

    fn validate_structural_invariants(&self) -> Result<(), HandInvariantError> {
        self.rules
            .validate()
            .map_err(|_| HandInvariantError::new("rules.invalid"))?;
        if self.rules.variant != self.progress.variant()
            || self.rules.variant != self.wall.variant()
        {
            return Err(HandInvariantError::new("variant.mismatch"));
        }
        let seat_count = usize::from(self.rules.variant.seat_count().value());
        if self.players.len() != seat_count || self.kan_counts.len() != seat_count {
            return Err(HandInvariantError::new("players.count"));
        }

        let mut counted_kans = 0_u8;
        let mut has_meld = false;
        for player in &self.players {
            if player.double_riichi && matches!(player.riichi, RiichiStatus::None) {
                return Err(HandInvariantError::new("riichi.double_without_riichi"));
            }
            let mut meld_ids = HashSet::with_capacity(player.melds.len());
            if player.nuki_tiles.len() > 4
                || player
                    .nuki_tiles
                    .iter()
                    .any(|tile| tile.kind() != TileKind::honor(crate::Honor::North))
            {
                return Err(HandInvariantError::new("nuki.tiles"));
            }
            for meld in &player.melds {
                has_meld = true;
                if !meld_ids.insert(meld.id()) {
                    return Err(HandInvariantError::new("meld.id_duplicate"));
                }
                let expected_tiles = match meld.kind() {
                    MeldKind::Chi | MeldKind::Pon => 3,
                    MeldKind::OpenKan | MeldKind::ConcealedKan | MeldKind::AddedKan => {
                        counted_kans = counted_kans
                            .checked_add(1)
                            .ok_or_else(|| HandInvariantError::new("kan.count_overflow"))?;
                        4
                    }
                };
                if meld.tiles().len() != expected_tiles {
                    return Err(HandInvariantError::new("meld.tile_count"));
                }
                match meld.kind() {
                    MeldKind::Chi | MeldKind::Pon | MeldKind::OpenKan | MeldKind::AddedKan => {
                        if meld.called_from().is_none() || meld.called_tile().is_none() {
                            return Err(HandInvariantError::new("meld.call_reference_missing"));
                        }
                    }
                    MeldKind::ConcealedKan => {
                        if meld.called_from().is_some() || meld.called_tile().is_some() {
                            return Err(HandInvariantError::new("meld.concealed_call_reference"));
                        }
                    }
                }
            }
        }
        let recorded_kans = self.kan_counts.iter().copied().sum::<u8>();
        let placed_kan_awaiting_resolution = matches!(
            self.phase,
            Phase::KanResponses(PendingKan::Concealed { .. } | PendingKan::Added { .. })
                | Phase::Ended(crate::EndReason::Ron)
        ) && recorded_kans.checked_add(1)
            == Some(counted_kans);
        if counted_kans != recorded_kans && !placed_kan_awaiting_resolution {
            return Err(HandInvariantError::new("kan.count_mismatch"));
        }
        let uncompleted_concealed_kan_is_only_meld = !self.calls_occurred
            && has_meld
            && recorded_kans == 0
            && counted_kans == 1
            && matches!(
                self.phase,
                Phase::KanResponses(PendingKan::Concealed { .. })
                    | Phase::Ended(crate::EndReason::Ron)
            );
        if self.calls_occurred != has_meld && !uncompleted_concealed_kan_is_only_meld {
            return Err(HandInvariantError::new("calls.flag_mismatch"));
        }

        let four_kans_aborted = matches!(self.phase, Phase::Ended(crate::EndReason::FourKans));
        let completed_kan_draws = recorded_kans.saturating_sub(u8::from(four_kans_aborted));
        let nuki_draws = self
            .players
            .iter()
            .map(|player| u8::try_from(player.nuki_tiles.len()).unwrap_or(u8::MAX))
            .sum::<u8>();
        let expected_rinshan_draws = completed_kan_draws + nuki_draws;
        let north_is_exposed_before_its_draw = matches!(
            self.phase,
            Phase::KanResponses(PendingKan::Nuki { .. }) | Phase::Ended(crate::EndReason::Ron)
        ) && self.wall.rinshan_draw_count().checked_add(1)
            == Some(expected_rinshan_draws);
        if self.wall.rinshan_draw_count() != expected_rinshan_draws
            && !north_is_exposed_before_its_draw
        {
            return Err(HandInvariantError::new("kan.rinshan_count"));
        }
        let expected_dora_count = if self.rules.bonuses.kan_dora {
            1_u8.checked_add(completed_kan_draws)
                .ok_or_else(|| HandInvariantError::new("kan.dora_count_overflow"))?
        } else {
            1
        };
        if self.wall.revealed_dora_count() != expected_dora_count {
            return Err(HandInvariantError::new("kan.dora_count"));
        }
        Ok(())
    }

    fn validate_tile_invariants(&self) -> Result<(), HandInvariantError> {
        let mut located = HashSet::with_capacity(
            self.wall.live_draw_count() + usize::from(self.wall.rinshan_draw_count()),
        );
        for player in &self.players {
            for tile in &player.concealed {
                self.register_tile(&mut located, *tile)?;
            }
            for meld in &player.melds {
                for tile in meld.tiles() {
                    self.register_tile(&mut located, *tile)?;
                }
            }
            for tile in &player.nuki_tiles {
                self.register_tile(&mut located, *tile)?;
            }
            for discard in &player.discards {
                let tile = discard.tile();
                self.validate_canonical_tile(tile)?;
                if discard.claimed_by().is_none() {
                    if !located.insert(tile.id()) {
                        return Err(HandInvariantError::new("tile.location_duplicate"));
                    }
                } else if !self.has_call_reference(tile.id(), discard.claimed_by()) {
                    return Err(HandInvariantError::new("discard.claim_reference"));
                }
            }
            if let Some(drawn_tile) = player.drawn_tile {
                if !player.concealed.iter().any(|tile| tile.id() == drawn_tile) {
                    return Err(HandInvariantError::new("drawn_tile.not_concealed"));
                }
            }
        }

        let expected_located =
            self.wall.live_draw_count() + usize::from(self.wall.rinshan_draw_count());
        if located.len() != expected_located {
            return Err(HandInvariantError::new("tile.located_count"));
        }
        Ok(())
    }

    fn validate_phase_invariants(&self) -> Result<(), HandInvariantError> {
        match &self.phase {
            Phase::TurnAction { seat, .. } => {
                self.ensure_valid_seat(*seat)?;
                self.ensure_only_drawn_tile(*seat)?;
            }
            Phase::DiscardAfterCall { seat, .. } => {
                self.ensure_valid_seat(*seat)?;
                self.ensure_no_drawn_tiles()?;
            }
            Phase::Responses(pending) => {
                self.ensure_valid_seat(pending.discarder)?;
                self.ensure_response_count(&pending.responses)?;
                self.ensure_no_drawn_tiles()?;
                let discard = self.players[usize::from(pending.discarder.index())]
                    .discards
                    .get(pending.discard_index)
                    .ok_or_else(|| HandInvariantError::new("phase.discard_missing"))?;
                if discard.claimed_by().is_some()
                    || !matches!(
                        pending.responses[usize::from(pending.discarder.index())],
                        Some(Reaction::Pass)
                    )
                {
                    return Err(HandInvariantError::new("phase.discard_response_owner"));
                }
                let player = &self.players[usize::from(pending.discarder.index())];
                if matches!(player.riichi, RiichiStatus::Pending) != discard.is_riichi_declaration()
                {
                    return Err(HandInvariantError::new("phase.pending_riichi"));
                }
            }
            Phase::KanResponses(pending) => {
                let declarer = pending.declarer();
                self.ensure_valid_seat(declarer)?;
                self.ensure_response_count(pending.responses())?;
                if !matches!(
                    pending.responses()[usize::from(declarer.index())],
                    Some(Reaction::Pass)
                ) {
                    return Err(HandInvariantError::new("phase.kan_response_owner"));
                }
                self.ensure_no_other_drawn_tiles(declarer)?;
                self.validate_pending_kan_tiles(pending)?;
            }
            Phase::Ended(_) => {}
        }
        Ok(())
    }

    fn register_tile(
        &self,
        located: &mut HashSet<TileId>,
        tile: Tile,
    ) -> Result<(), HandInvariantError> {
        self.validate_canonical_tile(tile)?;
        if located.insert(tile.id()) {
            Ok(())
        } else {
            Err(HandInvariantError::new("tile.location_duplicate"))
        }
    }

    fn validate_canonical_tile(&self, tile: Tile) -> Result<(), HandInvariantError> {
        if self.wall.tile_by_id(tile.id()) == Some(tile) {
            Ok(())
        } else {
            Err(HandInvariantError::new("tile.not_in_wall"))
        }
    }

    fn has_call_reference(&self, tile_id: TileId, claimed_by: Option<Seat>) -> bool {
        claimed_by.is_some_and(|seat| {
            self.players
                .get(usize::from(seat.index()))
                .is_some_and(|player| {
                    player
                        .melds
                        .iter()
                        .any(|meld| meld.called_tile() == Some(tile_id))
                })
        })
    }

    fn ensure_valid_seat(&self, seat: Seat) -> Result<(), HandInvariantError> {
        self.validate_seat(seat)
            .map_err(|_| HandInvariantError::new("phase.invalid_seat"))
    }

    fn ensure_response_count(
        &self,
        responses: &[Option<Reaction>],
    ) -> Result<(), HandInvariantError> {
        if responses.len() == self.players.len() {
            Ok(())
        } else {
            Err(HandInvariantError::new("phase.response_count"))
        }
    }

    fn ensure_no_drawn_tiles(&self) -> Result<(), HandInvariantError> {
        if self
            .players
            .iter()
            .all(|player| player.drawn_tile.is_none())
        {
            Ok(())
        } else {
            Err(HandInvariantError::new("phase.unexpected_drawn_tile"))
        }
    }

    fn ensure_only_drawn_tile(&self, seat: Seat) -> Result<(), HandInvariantError> {
        for (index, player) in self.players.iter().enumerate() {
            let should_have_drawn = index == usize::from(seat.index());
            if player.drawn_tile.is_some() != should_have_drawn {
                return Err(HandInvariantError::new("phase.drawn_tile_owner"));
            }
        }
        Ok(())
    }

    fn ensure_no_other_drawn_tiles(&self, seat: Seat) -> Result<(), HandInvariantError> {
        for (index, player) in self.players.iter().enumerate() {
            if index != usize::from(seat.index()) && player.drawn_tile.is_some() {
                return Err(HandInvariantError::new("phase.drawn_tile_owner"));
            }
        }
        Ok(())
    }

    fn validate_pending_kan_tiles(&self, pending: &PendingKan) -> Result<(), HandInvariantError> {
        let declarer = pending.declarer();
        let player = &self.players[usize::from(declarer.index())];
        match pending {
            PendingKan::Concealed { meld, .. } => {
                if matches!(meld.kind(), MeldKind::ConcealedKan)
                    && player.melds.iter().any(|placed| placed == meld)
                    && !meld.tiles().iter().any(|tile| {
                        player
                            .concealed
                            .iter()
                            .any(|concealed| concealed.id() == tile.id())
                    })
                {
                    Ok(())
                } else {
                    Err(HandInvariantError::new("phase.concealed_kan_tile"))
                }
            }
            PendingKan::Added {
                meld, added_tile, ..
            } => {
                if matches!(meld.kind(), MeldKind::AddedKan)
                    && meld.tiles().iter().any(|tile| tile == added_tile)
                    && player.melds.iter().any(|placed| placed == meld)
                    && !player
                        .concealed
                        .iter()
                        .any(|concealed| concealed.id() == added_tile.id())
                {
                    Ok(())
                } else {
                    Err(HandInvariantError::new("phase.added_kan_tile"))
                }
            }
            PendingKan::Nuki { tile, .. } => {
                if tile.kind() == TileKind::honor(crate::Honor::North)
                    && player.nuki_tiles.iter().any(|nuki| nuki == tile)
                    && !player
                        .concealed
                        .iter()
                        .any(|concealed| concealed.id() == tile.id())
                {
                    Ok(())
                } else {
                    Err(HandInvariantError::new("phase.nuki_tile"))
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HandInvariantError {
    code: &'static str,
}

impl HandInvariantError {
    const fn new(code: &'static str) -> Self {
        Self { code }
    }

    #[must_use]
    pub const fn code(self) -> &'static str {
        self.code
    }
}

impl Display for HandInvariantError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "riichi hand invariant '{}' is not satisfied",
            self.code
        )
    }
}

impl Error for HandInvariantError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HandCheckpointError {
    UnsupportedSchema { actual: u16, supported: u16 },
    InvalidState(HandInvariantError),
}

impl Display for HandCheckpointError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSchema { actual, supported } => write!(
                formatter,
                "unsupported hand checkpoint schema {actual}; supported schema is {supported}"
            ),
            Self::InvalidState(error) => Display::fmt(error, formatter),
        }
    }
}

impl Error for HandCheckpointError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidState(error) => Some(error),
            Self::UnsupportedSchema { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        HandPhase, Reaction, RejectAllHandJudge, RiichiHand, RiichiRules, RiichiVariant, Seat,
        TableProgress, WallSeed,
    };

    use super::{HAND_CHECKPOINT_SCHEMA_VERSION, HandCheckpointError};

    fn start(seed: u8) -> RiichiHand {
        let variant = RiichiVariant::Yonma;
        let dealer = Seat::new(variant, 0).expect("dealer");
        let progress = TableProgress::east_one(variant, dealer).expect("progress");
        RiichiHand::start(
            RiichiRules::standard(variant),
            progress,
            [25_000; 4],
            &WallSeed::from_bytes([seed; 32]),
        )
        .expect("start")
        .0
    }

    #[test]
    fn checkpoint_round_trip_preserves_state_and_event_sequence() {
        let mut hand = start(41);
        let dealer = Seat::new(RiichiVariant::Yonma, 0).expect("dealer");
        let discard = hand
            .player(dealer)
            .expect("dealer")
            .drawn_tile_id()
            .expect("drawn tile");
        hand.discard(dealer, discard).expect("discard");

        let checkpoint = hand.checkpoint(18).expect("valid checkpoint");
        assert_eq!(checkpoint.schema_version(), HAND_CHECKPOINT_SCHEMA_VERSION);
        assert_eq!(checkpoint.last_event_sequence(), 18);
        let debug = format!("{checkpoint:?}");
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("concealed"));

        let (restored, last_event_sequence) = checkpoint.restore().expect("restore");

        assert_eq!(restored, hand);
        assert_eq!(last_event_sequence, 18);
    }

    #[test]
    fn checkpoint_validates_called_tile_history() {
        let (mut hand, caller, hand_tiles, discard_id) = (0..=u8::MAX)
            .find_map(|seed| {
                let hand = start(seed);
                let dealer = Seat::new(RiichiVariant::Yonma, 0).expect("dealer");
                hand.player(dealer)
                    .expect("dealer")
                    .concealed()
                    .iter()
                    .find_map(|discard| {
                        (1..4).find_map(|index| {
                            let caller =
                                Seat::new(RiichiVariant::Yonma, index).expect("caller seat");
                            let matching: Vec<_> = hand
                                .player(caller)
                                .expect("caller")
                                .concealed()
                                .iter()
                                .filter(|tile| tile.kind() == discard.kind())
                                .map(|tile| tile.id())
                                .collect();
                            (matching.len() >= 2).then(|| {
                                (
                                    hand.clone(),
                                    caller,
                                    [matching[0], matching[1]],
                                    discard.id(),
                                )
                            })
                        })
                    })
            })
            .expect("fixed seed range contains a pon opportunity");
        let dealer = Seat::new(RiichiVariant::Yonma, 0).expect("dealer");
        hand.discard(dealer, discard_id).expect("discard");
        hand.respond(caller, Reaction::Pon { hand_tiles }, &RejectAllHandJudge)
            .expect("pon");
        for index in 1..4 {
            let seat = Seat::new(RiichiVariant::Yonma, index).expect("seat");
            if seat != caller {
                hand.pass(seat, &RejectAllHandJudge).expect("pass");
            }
        }

        assert_eq!(hand.phase(), HandPhase::AwaitingDiscard { seat: caller });
        hand.checkpoint(22).expect("called state is valid");
    }

    #[test]
    fn checkpoint_validates_completed_kan_counters() {
        let (mut hand, caller, hand_tiles, discard_id) = (0..=u8::MAX)
            .find_map(|seed| {
                let hand = start(seed);
                let dealer = Seat::new(RiichiVariant::Yonma, 0).expect("dealer");
                hand.player(dealer)
                    .expect("dealer")
                    .concealed()
                    .iter()
                    .find_map(|discard| {
                        (1..4).find_map(|index| {
                            let caller =
                                Seat::new(RiichiVariant::Yonma, index).expect("caller seat");
                            let matching: Vec<_> = hand
                                .player(caller)
                                .expect("caller")
                                .concealed()
                                .iter()
                                .filter(|tile| tile.kind() == discard.kind())
                                .map(|tile| tile.id())
                                .collect();
                            (matching.len() >= 3).then(|| {
                                (
                                    hand.clone(),
                                    caller,
                                    [matching[0], matching[1], matching[2]],
                                    discard.id(),
                                )
                            })
                        })
                    })
            })
            .expect("fixed seed range contains an open-kan opportunity");
        let dealer = Seat::new(RiichiVariant::Yonma, 0).expect("dealer");
        hand.discard(dealer, discard_id).expect("discard");
        hand.respond(
            caller,
            Reaction::OpenKan { hand_tiles },
            &RejectAllHandJudge,
        )
        .expect("open kan");
        for index in 1..4 {
            let seat = Seat::new(RiichiVariant::Yonma, index).expect("seat");
            if seat != caller {
                hand.pass(seat, &RejectAllHandJudge).expect("pass");
            }
        }

        assert_eq!(hand.phase(), HandPhase::AwaitingTurnAction { seat: caller });
        hand.checkpoint(26).expect("kan state is valid");
    }

    #[test]
    fn checkpoint_rejects_duplicate_physical_tile() {
        let mut hand = start(42);
        hand.players[1].concealed[0] = hand.players[0].concealed[0];

        let error = hand.checkpoint(0).expect_err("corrupt state");

        assert_eq!(error.code(), "tile.location_duplicate");
    }

    #[test]
    fn restore_rejects_unknown_schema_and_corrupt_state() {
        let hand = start(43);
        let mut unsupported = hand.checkpoint(7).expect("checkpoint");
        unsupported.schema_version = HAND_CHECKPOINT_SCHEMA_VERSION + 1;
        assert!(matches!(
            unsupported.restore(),
            Err(HandCheckpointError::UnsupportedSchema { .. })
        ));

        let mut corrupt = hand.checkpoint(7).expect("checkpoint");
        corrupt.state.kan_counts[0] = 1;
        assert!(matches!(
            corrupt.restore(),
            Err(HandCheckpointError::InvalidState(_))
        ));
    }
}
