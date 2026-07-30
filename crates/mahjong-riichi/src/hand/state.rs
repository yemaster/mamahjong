use std::error::Error;
use std::fmt::{self, Display, Formatter};

use crate::{
    Discard, DrawSource, EndReason, HandEvent, HandPhase, HandTransition, PlayerHand, ReactionKind,
    RiichiRules, RiichiVariant, Seat, TableProgress, TileId, TileSet, TileSetError,
    ValidationErrors, Wall, WallSeed,
};

const INITIAL_CONCEALED_TILES: usize = 13;

#[derive(Clone, Debug, Eq, PartialEq)]
struct PendingDiscard {
    discarder: Seat,
    discard_index: usize,
    passed: Box<[bool]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Phase {
    TurnAction { seat: Seat },
    Responses(PendingDiscard),
    Ended(EndReason),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiichiHand {
    rules: RiichiRules,
    progress: TableProgress,
    wall: Wall,
    players: Box<[PlayerHand]>,
    phase: Phase,
}

impl RiichiHand {
    pub fn start(
        rules: RiichiRules,
        progress: TableProgress,
        points: impl IntoIterator<Item = i32>,
        seed: &WallSeed,
    ) -> Result<(Self, HandTransition), HandError> {
        rules
            .validate()
            .map_err(HandError::InvalidRuleConfiguration)?;
        if rules.variant != progress.variant() {
            return Err(HandError::VariantMismatch {
                rules: rules.variant,
                progress: progress.variant(),
            });
        }

        let points: Vec<_> = points.into_iter().collect();
        let expected_players = usize::from(rules.variant.seat_count().value());
        if points.len() != expected_players {
            return Err(HandError::PointCount {
                expected: expected_players,
                actual: points.len(),
            });
        }

        let tile_set =
            TileSet::new(rules.variant, rules.bonuses.red_fives).map_err(HandError::TileSet)?;
        let mut wall = Wall::new(tile_set, seed);
        let mut players: Vec<_> = points.into_iter().map(PlayerHand::new).collect();
        let dealer = progress.dealer();

        for _ in 0..INITIAL_CONCEALED_TILES {
            for offset in 0..rules.variant.seat_count().value() {
                let seat = seat_at_offset(rules.variant, dealer, offset);
                let tile = wall
                    .draw_live()
                    .expect("validated riichi wall contains all initial deal tiles");
                players[usize::from(seat.index())].concealed.push(tile);
            }
        }

        let dora_indicator = wall
            .current_dora_indicators()
            .next()
            .expect("a riichi wall always starts with one dora indicator");
        let mut events = Vec::with_capacity(expected_players + 2);
        events.push(HandEvent::HandStarted {
            progress,
            dora_indicator,
            remaining_live_draws: wall.remaining_live_draws(),
        });
        for offset in 0..rules.variant.seat_count().value() {
            let seat = seat_at_offset(rules.variant, dealer, offset);
            events.push(HandEvent::InitialHandDealt {
                seat,
                tiles: players[usize::from(seat.index())]
                    .concealed
                    .clone()
                    .into_boxed_slice(),
            });
        }

        let dealer_draw = wall
            .draw_live()
            .expect("validated riichi wall contains the dealer's first draw");
        let dealer_hand = &mut players[usize::from(dealer.index())];
        dealer_hand.concealed.push(dealer_draw);
        dealer_hand.drawn_tile = Some(dealer_draw.id());
        events.push(HandEvent::TileDrawn {
            seat: dealer,
            tile: dealer_draw,
            source: DrawSource::LiveWall,
            remaining_live_draws: wall.remaining_live_draws(),
        });

        Ok((
            Self {
                rules,
                progress,
                wall,
                players: players.into_boxed_slice(),
                phase: Phase::TurnAction { seat: dealer },
            },
            HandTransition::new(events),
        ))
    }

    #[must_use]
    pub const fn rules(&self) -> &RiichiRules {
        &self.rules
    }

    #[must_use]
    pub const fn progress(&self) -> TableProgress {
        self.progress
    }

    #[must_use]
    pub fn remaining_live_draws(&self) -> usize {
        self.wall.remaining_live_draws()
    }

    #[must_use]
    pub fn phase(&self) -> HandPhase {
        match self.phase {
            Phase::TurnAction { seat } => HandPhase::AwaitingTurnAction { seat },
            Phase::Responses(ref pending) => HandPhase::AwaitingResponses {
                trigger_seat: pending.discarder,
            },
            Phase::Ended(reason) => HandPhase::Ended { reason },
        }
    }

    pub fn player(&self, seat: Seat) -> Result<&PlayerHand, HandError> {
        self.validate_seat(seat)?;
        Ok(&self.players[usize::from(seat.index())])
    }

    pub fn discard(&mut self, actor: Seat, tile_id: TileId) -> Result<HandTransition, HandError> {
        self.validate_seat(actor)?;
        let active_seat = match self.phase {
            Phase::TurnAction { seat } => seat,
            _ => return Err(HandError::WrongPhase),
        };
        if actor != active_seat {
            return Err(HandError::NotActiveSeat {
                expected: active_seat,
                actual: actor,
            });
        }

        let player = &self.players[usize::from(actor.index())];
        let tile_index = player
            .concealed
            .iter()
            .position(|tile| tile.id() == tile_id)
            .ok_or(HandError::TileNotInHand { tile_id })?;
        let tsumogiri = player.drawn_tile == Some(tile_id);

        let player = &mut self.players[usize::from(actor.index())];
        let tile = player.concealed.swap_remove(tile_index);
        player.drawn_tile = None;
        let discard_index = player.discards.len();
        player.discards.push(Discard::new(tile, tsumogiri));
        let mut passed =
            vec![false; usize::from(self.rules.variant.seat_count().value())].into_boxed_slice();
        passed[usize::from(actor.index())] = true;
        self.phase = Phase::Responses(PendingDiscard {
            discarder: actor,
            discard_index,
            passed,
        });

        Ok(HandTransition::new(vec![HandEvent::TileDiscarded {
            seat: actor,
            tile,
            tsumogiri,
            riichi_declared: false,
        }]))
    }

    pub fn pass(&mut self, actor: Seat) -> Result<HandTransition, HandError> {
        self.validate_seat(actor)?;
        let all_passed = {
            let Phase::Responses(pending) = &mut self.phase else {
                return Err(HandError::WrongPhase);
            };
            if actor == pending.discarder {
                return Err(HandError::DiscarderCannotReact);
            }
            let actor_passed = &mut pending.passed[usize::from(actor.index())];
            if *actor_passed {
                return Err(HandError::AlreadyResponded { seat: actor });
            }
            *actor_passed = true;
            pending.passed.iter().all(|passed| *passed)
        };

        let mut transition = HandTransition::new(vec![HandEvent::ReactionSubmitted {
            seat: actor,
            reaction: ReactionKind::Pass,
        }]);
        if all_passed {
            transition.append(self.resolve_unclaimed_discard()?);
        }
        Ok(transition)
    }

    fn resolve_unclaimed_discard(&mut self) -> Result<HandTransition, HandError> {
        let pending = match &self.phase {
            Phase::Responses(pending) => pending.clone(),
            _ => return Err(HandError::WrongPhase),
        };
        debug_assert!(
            self.players[usize::from(pending.discarder.index())]
                .discards
                .get(pending.discard_index)
                .is_some()
        );

        if self.wall.remaining_live_draws() == 0 {
            self.phase = Phase::Ended(EndReason::ExhaustiveDraw);
            return Ok(HandTransition::new(vec![
                HandEvent::ExhaustiveDrawDeclared {
                    reason: EndReason::ExhaustiveDraw,
                },
            ]));
        }

        let next_seat = seat_at_offset(self.rules.variant, pending.discarder, 1);
        let tile = self
            .wall
            .draw_live()
            .expect("remaining-live-draw check guarantees a tile");
        let next_player = &mut self.players[usize::from(next_seat.index())];
        next_player.concealed.push(tile);
        next_player.drawn_tile = Some(tile.id());
        next_player.temporary_furiten = false;
        self.phase = Phase::TurnAction { seat: next_seat };

        Ok(HandTransition::new(vec![HandEvent::TileDrawn {
            seat: next_seat,
            tile,
            source: DrawSource::LiveWall,
            remaining_live_draws: self.wall.remaining_live_draws(),
        }]))
    }

    fn validate_seat(&self, seat: Seat) -> Result<(), HandError> {
        let seat_count = self.rules.variant.seat_count().value();
        if seat.index() < seat_count {
            Ok(())
        } else {
            Err(HandError::InvalidSeat {
                index: seat.index(),
                seat_count,
            })
        }
    }
}

fn seat_at_offset(variant: RiichiVariant, seat: Seat, offset: u8) -> Seat {
    let index = (seat.index() + offset) % variant.seat_count().value();
    Seat::new(variant, index).expect("modulo seat count always produces a valid seat")
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HandError {
    InvalidRuleConfiguration(ValidationErrors),
    VariantMismatch {
        rules: RiichiVariant,
        progress: RiichiVariant,
    },
    PointCount {
        expected: usize,
        actual: usize,
    },
    TileSet(TileSetError),
    InvalidSeat {
        index: u8,
        seat_count: u8,
    },
    WrongPhase,
    NotActiveSeat {
        expected: Seat,
        actual: Seat,
    },
    TileNotInHand {
        tile_id: TileId,
    },
    DiscarderCannotReact,
    AlreadyResponded {
        seat: Seat,
    },
}

impl Display for HandError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRuleConfiguration(errors) => Display::fmt(errors, formatter),
            Self::VariantMismatch { rules, progress } => {
                write!(
                    formatter,
                    "rule variant {rules:?} does not match progress variant {progress:?}"
                )
            }
            Self::PointCount { expected, actual } => {
                write!(formatter, "expected {expected} point entries, got {actual}")
            }
            Self::TileSet(error) => Display::fmt(error, formatter),
            Self::InvalidSeat { index, seat_count } => {
                write!(
                    formatter,
                    "seat index {index} is outside a {seat_count}-player hand"
                )
            }
            Self::WrongPhase => formatter.write_str("command is not legal in the current phase"),
            Self::NotActiveSeat { expected, actual } => {
                write!(
                    formatter,
                    "seat {} cannot act; current seat is {}",
                    actual.index(),
                    expected.index()
                )
            }
            Self::TileNotInHand { tile_id } => {
                write!(
                    formatter,
                    "tile {} is not in the actor's hand",
                    tile_id.value()
                )
            }
            Self::DiscarderCannotReact => {
                formatter.write_str("the discarder cannot react to their own tile")
            }
            Self::AlreadyResponded { seat } => {
                write!(formatter, "seat {} already responded", seat.index())
            }
        }
    }
}

impl Error for HandError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidRuleConfiguration(errors) => Some(errors),
            Self::TileSet(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::{
        EndReason, HandEvent, HandPhase, RiichiHand, RiichiRules, RiichiVariant, Seat,
        TableProgress, TileId, WallSeed,
    };

    fn start_hand(variant: RiichiVariant, dealer_index: u8) -> RiichiHand {
        let dealer = Seat::new(variant, dealer_index).expect("dealer");
        let progress = TableProgress::east_one(variant, dealer).expect("progress");
        let points = vec![25_000; usize::from(variant.seat_count().value())];
        RiichiHand::start(
            RiichiRules::standard(variant),
            progress,
            points,
            &WallSeed::from_bytes([7; 32]),
        )
        .expect("start hand")
        .0
    }

    fn pass_all(hand: &mut RiichiHand, discarder: Seat) -> crate::HandTransition {
        let variant = hand.rules().variant;
        let mut last = None;
        for offset in 1..variant.seat_count().value() {
            let seat = super::seat_at_offset(variant, discarder, offset);
            last = Some(hand.pass(seat).expect("pass"));
        }
        last.expect("a riichi hand always has opponents")
    }

    #[test]
    fn yonma_deal_assigns_every_tile_once_and_draws_for_dealer() {
        let dealer = Seat::new(RiichiVariant::Yonma, 2).expect("dealer");
        let progress = TableProgress::east_one(RiichiVariant::Yonma, dealer).expect("progress");
        let (hand, transition) = RiichiHand::start(
            RiichiRules::default(),
            progress,
            [25_000; 4],
            &WallSeed::from_bytes([3; 32]),
        )
        .expect("start");

        assert_eq!(hand.phase(), HandPhase::AwaitingTurnAction { seat: dealer });
        assert_eq!(hand.remaining_live_draws(), 69);
        assert_eq!(transition.events().len(), 6);
        assert!(matches!(
            transition.events()[0],
            HandEvent::HandStarted { .. }
        ));

        let mut ids = HashSet::new();
        for index in 0..4 {
            let seat = Seat::new(RiichiVariant::Yonma, index).expect("seat");
            let player = hand.player(seat).expect("player");
            let expected = if seat == dealer { 14 } else { 13 };
            assert_eq!(player.concealed().len(), expected);
            for tile in player.concealed() {
                assert!(ids.insert(tile.id()));
            }
        }
        assert_eq!(ids.len(), 53);
    }

    #[test]
    fn sanma_deal_has_expected_live_wall_capacity() {
        let hand = start_hand(RiichiVariant::Sanma, 1);

        assert_eq!(hand.remaining_live_draws(), 54);
        for index in 0..3 {
            let seat = Seat::new(RiichiVariant::Sanma, index).expect("seat");
            let expected = if index == 1 { 14 } else { 13 };
            assert_eq!(
                hand.player(seat).expect("player").concealed().len(),
                expected
            );
        }
    }

    #[test]
    fn discard_then_unclaimed_resolution_draws_for_next_seat() {
        let mut hand = start_hand(RiichiVariant::Yonma, 3);
        let dealer = Seat::new(RiichiVariant::Yonma, 3).expect("dealer");
        let drawn = hand
            .player(dealer)
            .expect("dealer")
            .drawn_tile_id()
            .expect("drawn tile");

        let discarded = hand.discard(dealer, drawn).expect("discard");

        assert!(matches!(
            discarded.events(),
            [HandEvent::TileDiscarded {
                seat,
                tsumogiri: true,
                ..
            }] if *seat == dealer
        ));
        assert_eq!(
            hand.phase(),
            HandPhase::AwaitingResponses {
                trigger_seat: dealer
            }
        );

        let next = Seat::new(RiichiVariant::Yonma, 0).expect("next");
        let drawn = pass_all(&mut hand, dealer);
        assert!(matches!(
            drawn.events().last(),
            Some(HandEvent::TileDrawn { seat, .. }) if *seat == next
        ));
        assert_eq!(hand.phase(), HandPhase::AwaitingTurnAction { seat: next });
        assert_eq!(hand.player(dealer).expect("dealer").concealed().len(), 13);
        assert_eq!(hand.player(next).expect("next").concealed().len(), 14);
    }

    #[test]
    fn invalid_discard_does_not_mutate_hand() {
        let mut hand = start_hand(RiichiVariant::Yonma, 0);
        let dealer = Seat::new(RiichiVariant::Yonma, 0).expect("dealer");
        let other = Seat::new(RiichiVariant::Yonma, 1).expect("other");
        let before = hand.clone();

        assert!(hand.discard(other, TileId::new(u16::MAX)).is_err());
        assert_eq!(hand, before);

        assert!(hand.discard(dealer, TileId::new(u16::MAX)).is_err());
        assert_eq!(hand, before);
    }

    #[test]
    fn duplicate_or_self_pass_does_not_mutate_response_window() {
        let mut hand = start_hand(RiichiVariant::Yonma, 0);
        let dealer = Seat::new(RiichiVariant::Yonma, 0).expect("dealer");
        let responder = Seat::new(RiichiVariant::Yonma, 1).expect("responder");
        let drawn = hand
            .player(dealer)
            .expect("dealer")
            .drawn_tile_id()
            .expect("draw");
        hand.discard(dealer, drawn).expect("discard");

        let before_self_pass = hand.clone();
        assert!(hand.pass(dealer).is_err());
        assert_eq!(hand, before_self_pass);

        hand.pass(responder).expect("first pass");
        let after_first_pass = hand.clone();
        assert!(hand.pass(responder).is_err());
        assert_eq!(hand, after_first_pass);
    }

    #[test]
    fn setup_rejects_variant_or_point_count_mismatch() {
        let yonma_dealer = Seat::new(RiichiVariant::Yonma, 0).expect("dealer");
        let yonma_progress =
            TableProgress::east_one(RiichiVariant::Yonma, yonma_dealer).expect("progress");
        let seed = WallSeed::from_bytes([8; 32]);

        assert!(
            RiichiHand::start(
                RiichiRules::standard(RiichiVariant::Sanma),
                yonma_progress,
                [25_000; 3],
                &seed,
            )
            .is_err()
        );
        assert!(
            RiichiHand::start(RiichiRules::default(), yonma_progress, [25_000; 3], &seed).is_err()
        );
    }

    #[test]
    fn exhausting_live_wall_ends_after_last_discard() {
        let mut hand = start_hand(RiichiVariant::Sanma, 0);

        loop {
            let HandPhase::AwaitingTurnAction { seat } = hand.phase() else {
                panic!("expected turn");
            };
            let drawn = hand
                .player(seat)
                .expect("player")
                .drawn_tile_id()
                .expect("draw");
            hand.discard(seat, drawn).expect("discard");
            let transition = pass_all(&mut hand, seat);
            if matches!(
                transition.events().last(),
                Some(HandEvent::ExhaustiveDrawDeclared {
                    reason: EndReason::ExhaustiveDraw
                })
            ) {
                break;
            }
        }

        assert_eq!(
            hand.phase(),
            HandPhase::Ended {
                reason: EndReason::ExhaustiveDraw
            }
        );
        assert_eq!(hand.remaining_live_draws(), 0);
    }
}
