use std::error::Error;
use std::fmt::{self, Display, Formatter};

use crate::{
    Discard, DrawSource, EndReason, HandEvent, HandJudge, HandPhase, HandTransition, PlayerHand,
    Reaction, RiichiRules, RiichiScorer, RiichiVariant, Seat, TableProgress, Tile, TileFace,
    TileId, TileKind, TileSet, TileSetError, ValidationErrors, Wall, WallSeed, WinQuery, WinSource,
};

const INITIAL_CONCEALED_TILES: usize = 13;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PendingDiscard {
    pub(super) discarder: Seat,
    pub(super) discard_index: usize,
    pub(super) responses: Box<[Option<Reaction>]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum PendingKan {
    Concealed {
        declarer: Seat,
        meld: crate::Meld,
        responses: Box<[Option<Reaction>]>,
    },
    Added {
        declarer: Seat,
        meld: crate::Meld,
        added_tile: Tile,
        responses: Box<[Option<Reaction>]>,
    },
    Nuki {
        declarer: Seat,
        tile: Tile,
        responses: Box<[Option<Reaction>]>,
    },
}

impl PendingKan {
    pub(super) const fn declarer(&self) -> Seat {
        match self {
            Self::Concealed { declarer, .. }
            | Self::Added { declarer, .. }
            | Self::Nuki { declarer, .. } => *declarer,
        }
    }

    pub(super) fn responses(&self) -> &[Option<Reaction>] {
        match self {
            Self::Concealed { responses, .. }
            | Self::Added { responses, .. }
            | Self::Nuki { responses, .. } => responses,
        }
    }

    pub(super) fn responses_mut(&mut self) -> &mut [Option<Reaction>] {
        match self {
            Self::Concealed { responses, .. }
            | Self::Added { responses, .. }
            | Self::Nuki { responses, .. } => responses,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Phase {
    TurnAction {
        seat: Seat,
        draw_source: DrawSource,
    },
    Responses(PendingDiscard),
    KanResponses(PendingKan),
    DiscardAfterCall {
        seat: Seat,
        forbidden_discards: Box<[TileKind]>,
    },
    Ended(EndReason),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiichiHand {
    pub(super) rules: RiichiRules,
    pub(super) progress: TableProgress,
    pub(super) wall: Wall,
    pub(super) players: Box<[PlayerHand]>,
    pub(super) phase: Phase,
    pub(super) kan_counts: Box<[u8]>,
    pub(super) calls_occurred: bool,
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
        let mut wall = Wall::new_with_north_rule(tile_set, seed, rules.match_rules.north);
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
                phase: Phase::TurnAction {
                    seat: dealer,
                    draw_source: DrawSource::LiveWall,
                },
                kan_counts: vec![0; expected_players].into_boxed_slice(),
                calls_occurred: false,
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
    pub const fn completed_rinshan_draws(&self) -> u8 {
        self.wall.rinshan_draw_count()
    }

    /// 本局牌山洗好之后的完整顺序，以及活牌区末尾（王牌起点）的下标。
    ///
    /// 只给牌谱用，且只有对局结束之后才允许下发，详见 `Wall::ordered_tiles`。
    #[must_use]
    pub fn wall_order(&self) -> (&[crate::Tile], usize) {
        (self.wall.ordered_tiles(), self.wall.live_end())
    }

    #[must_use]
    pub fn current_dora_indicators(&self) -> impl ExactSizeIterator<Item = crate::Tile> + '_ {
        self.wall.current_dora_indicators()
    }

    #[must_use]
    pub fn matching_ura_dora_indicators(&self) -> impl ExactSizeIterator<Item = crate::Tile> + '_ {
        self.wall.matching_ura_dora_indicators()
    }

    #[must_use]
    pub fn phase(&self) -> HandPhase {
        match self.phase {
            Phase::TurnAction { seat, .. } => HandPhase::AwaitingTurnAction { seat },
            Phase::DiscardAfterCall { seat, .. } => HandPhase::AwaitingDiscard { seat },
            Phase::Responses(ref pending) => HandPhase::AwaitingResponses {
                trigger_seat: pending.discarder,
            },
            Phase::KanResponses(ref pending) => HandPhase::AwaitingResponses {
                trigger_seat: pending.declarer(),
            },
            Phase::Ended(reason) => HandPhase::Ended { reason },
        }
    }

    pub fn player(&self, seat: Seat) -> Result<&PlayerHand, HandError> {
        self.validate_seat(seat)?;
        Ok(&self.players[usize::from(seat.index())])
    }

    pub fn waiting_tile_hints(&self, seat: Seat) -> Result<Box<[(TileKind, bool)]>, HandError> {
        let player = self.player(seat)?;
        let scorer = RiichiScorer;
        let source = WinSource::Discard {
            from: seat_at_offset(self.rules.variant, seat, 1),
        };
        Ok(scorer
            .waiting_tiles(player)
            .kinds()
            .iter()
            .copied()
            .enumerate()
            .map(|(index, kind)| {
                let tile = Tile::new(
                    TileId::new(60_000 + u16::try_from(index).expect("wait index fits u16")),
                    kind,
                    false,
                )
                .expect("non-red waiting tile is valid");
                let has_yaku = scorer
                    .evaluate(WinQuery::new(
                        &self.rules,
                        self.progress,
                        seat,
                        player,
                        tile,
                        source,
                        &self.wall,
                        self.calls_occurred,
                    ))
                    .is_some();
                (kind, has_yaku)
            })
            .collect::<Vec<_>>()
            .into_boxed_slice())
    }

    pub fn is_furiten(&self, seat: Seat) -> Result<bool, HandError> {
        let player = self.player(seat)?;
        let waits = RiichiScorer.waiting_tiles(player);
        let discard_furiten = player
            .discards()
            .iter()
            .any(|discard| waits.contains(discard.tile().kind()));
        Ok(!waits.is_empty()
            && (discard_furiten || player.is_temporary_furiten() || player.is_riichi_furiten()))
    }

    /// 开发/测试专用：把某个座位的暗手整体换成给定牌码。牌 id 保持不变（含刚摸上来的那张，
    /// 它的 `drawn_tile` 指向不变），所以改动不会让「新摸的牌」凭空消失，只是换牌面。暗手
    /// 有几张就收几张（副露之后会更少）。这里只改牌面，不校验整场牌数一致性——正常对局不走
    /// 这条，纯粹是给手工测各种胡牌牌型留的后门。
    pub fn set_concealed_tiles(&mut self, seat: Seat, codes: &[String]) -> Result<(), HandError> {
        self.validate_seat(seat)?;
        let player = &mut self.players[usize::from(seat.index())];
        if codes.len() != player.concealed.len() {
            return Err(HandError::WrongConcealedTileCount {
                expected: player.concealed.len(),
                actual: codes.len(),
            });
        }
        let faces = codes
            .iter()
            .map(|code| {
                code.parse::<TileFace>()
                    .map_err(|_| HandError::InvalidTileCode(code.clone()))
            })
            .collect::<Result<Vec<_>, _>>()?;

        for (tile, face) in player.concealed.iter_mut().zip(faces) {
            *tile = Tile::new(tile.id(), face.kind(), face.is_red())
                .map_err(|error| HandError::InvalidTileCode(error.to_string()))?;
        }
        Ok(())
    }

    pub fn discard(&mut self, actor: Seat, tile_id: TileId) -> Result<HandTransition, HandError> {
        self.discard_internal(actor, tile_id, false)
    }

    pub(super) fn discard_internal(
        &mut self,
        actor: Seat,
        tile_id: TileId,
        declare_riichi: bool,
    ) -> Result<HandTransition, HandError> {
        self.validate_seat(actor)?;
        let (active_seat, forbidden_discards) = match &self.phase {
            Phase::TurnAction { seat, .. } => (*seat, None),
            Phase::DiscardAfterCall {
                seat,
                forbidden_discards,
            } => (*seat, Some(forbidden_discards.as_ref())),
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
        if matches!(player.riichi, crate::RiichiStatus::Established) && !tsumogiri {
            return Err(HandError::RiichiMustTsumogiri);
        }
        let tile_kind = player.concealed[tile_index].kind();
        if forbidden_discards.is_some_and(|forbidden| forbidden.contains(&tile_kind)) {
            return Err(HandError::ForbiddenDiscardAfterCall { kind: tile_kind });
        }

        let player = &mut self.players[usize::from(actor.index())];
        let ippatsu_expired =
            matches!(player.riichi, crate::RiichiStatus::Established) && player.ippatsu_eligible;
        let tile = player.concealed.swap_remove(tile_index);
        player.drawn_tile = None;
        player.ippatsu_eligible = false;
        if declare_riichi {
            player.riichi = crate::RiichiStatus::Pending;
        }
        let forced_sideways =
            matches!(player.riichi, crate::RiichiStatus::Pending) && !declare_riichi && !tsumogiri;
        let discard_index = player.discards.len();
        player.discards.push(Discard::new(
            tile,
            tsumogiri,
            declare_riichi || forced_sideways,
        ));
        let mut responses =
            vec![None; usize::from(self.rules.variant.seat_count().value())].into_boxed_slice();
        responses[usize::from(actor.index())] = Some(Reaction::Pass);
        self.phase = Phase::Responses(PendingDiscard {
            discarder: actor,
            discard_index,
            responses,
        });

        let mut events = vec![HandEvent::TileDiscarded {
            seat: actor,
            tile,
            tsumogiri,
            riichi_declared: declare_riichi,
        }];
        if ippatsu_expired {
            events.push(HandEvent::IppatsuExpired { seat: actor });
        }
        Ok(HandTransition::new(events))
    }

    pub(super) fn resolve_unclaimed_discard(
        &mut self,
        pending: &PendingDiscard,
        judge: &dyn HandJudge,
    ) -> HandTransition {
        debug_assert!(
            self.players[usize::from(pending.discarder.index())]
                .discards
                .get(pending.discard_index)
                .is_some()
        );

        if self.wall.remaining_live_draws() == 0 {
            let tenpai = (0..self.rules.variant.seat_count().value())
                .filter_map(|index| {
                    let seat = Seat::new(self.rules.variant, index).expect("valid seat index");
                    judge
                        .is_tenpai(&self.rules, &self.players[usize::from(seat.index())], seat)
                        .then_some(seat)
                })
                .collect::<Vec<_>>()
                .into_boxed_slice();
            self.phase = Phase::Ended(EndReason::ExhaustiveDraw);
            return HandTransition::new(vec![HandEvent::ExhaustiveDrawDeclared {
                reason: EndReason::ExhaustiveDraw,
                tenpai,
            }]);
        }

        let next_seat = seat_at_offset(self.rules.variant, pending.discarder, 1);
        let tile = self
            .wall
            .draw_live()
            .expect("remaining-live-draw check guarantees a tile");
        let next_player = &mut self.players[usize::from(next_seat.index())];
        let cleared_temporary_furiten = next_player.temporary_furiten;
        next_player.concealed.push(tile);
        next_player.drawn_tile = Some(tile.id());
        next_player.temporary_furiten = false;
        self.phase = Phase::TurnAction {
            seat: next_seat,
            draw_source: DrawSource::LiveWall,
        };

        let mut events = Vec::with_capacity(2);
        if cleared_temporary_furiten {
            events.push(HandEvent::FuritenChanged {
                seat: next_seat,
                temporary: false,
                riichi: next_player.riichi_furiten,
            });
        }
        events.push(HandEvent::TileDrawn {
            seat: next_seat,
            tile,
            source: DrawSource::LiveWall,
            remaining_live_draws: self.wall.remaining_live_draws(),
        });
        HandTransition::new(events)
    }

    pub(super) fn validate_seat(&self, seat: Seat) -> Result<(), HandError> {
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

pub(super) fn seat_at_offset(variant: RiichiVariant, seat: Seat, offset: u8) -> Seat {
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
    ForbiddenDiscardAfterCall {
        kind: TileKind,
    },
    RiichiMustTsumogiri,
    RiichiNotAllowed {
        reason: &'static str,
    },
    WinNotAllowed,
    AbortiveDrawNotAllowed {
        reason: &'static str,
    },
    NukiNotAllowed {
        reason: &'static str,
    },
    DuplicateTileSelection,
    InvalidReaction {
        reason: &'static str,
    },
    CannotCallOnLastDiscard,
    CannotCallWhileRiichi,
    KanNotAllowedOnLastDraw,
    KanLimitReached,
    MeldNotFound {
        meld_id: crate::MeldId,
    },
    MeldCannotBeAddedKan {
        meld_id: crate::MeldId,
    },
    DiscarderCannotReact,
    DeclarerCannotReact,
    AlreadyResponded {
        seat: Seat,
    },
    InvalidTileCode(String),
    WrongConcealedTileCount {
        expected: usize,
        actual: usize,
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
            Self::ForbiddenDiscardAfterCall { kind } => {
                write!(
                    formatter,
                    "tile kind {kind} is forbidden by call replacement rules"
                )
            }
            Self::RiichiMustTsumogiri => {
                formatter.write_str("an established riichi hand must discard its drawn tile")
            }
            Self::RiichiNotAllowed { reason } => formatter.write_str(reason),
            Self::WinNotAllowed => {
                formatter.write_str("the hand judge rejected the win declaration")
            }
            Self::AbortiveDrawNotAllowed { reason } => formatter.write_str(reason),
            Self::NukiNotAllowed { reason } => formatter.write_str(reason),
            Self::DuplicateTileSelection => {
                formatter.write_str("the same physical tile was selected more than once")
            }
            Self::InvalidReaction { reason } => formatter.write_str(reason),
            Self::CannotCallOnLastDiscard => {
                formatter.write_str("chi, pon, and kan are unavailable on the last discard")
            }
            Self::CannotCallWhileRiichi => {
                formatter.write_str("an established riichi hand cannot call another player's tile")
            }
            Self::KanNotAllowedOnLastDraw => {
                formatter.write_str("kan is unavailable when the live wall cannot supply a draw")
            }
            Self::KanLimitReached => {
                formatter.write_str("a hand cannot contain more than four kans")
            }
            Self::MeldNotFound { meld_id } => {
                write!(formatter, "meld {} does not exist", meld_id.value())
            }
            Self::MeldCannotBeAddedKan { meld_id } => {
                write!(formatter, "meld {} is not an open pon", meld_id.value())
            }
            Self::DiscarderCannotReact => {
                formatter.write_str("the discarder cannot react to their own tile")
            }
            Self::DeclarerCannotReact => {
                formatter.write_str("the kan declarer cannot react to their own kan")
            }
            Self::AlreadyResponded { seat } => {
                write!(formatter, "seat {} already responded", seat.index())
            }
            Self::InvalidTileCode(code) => write!(formatter, "invalid tile code {code}"),
            Self::WrongConcealedTileCount { expected, actual } => {
                write!(
                    formatter,
                    "expected {expected} concealed tiles, got {actual}"
                )
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
        EndReason, HandEvent, HandJudge, HandPhase, PlayerHand, RejectAllHandJudge, RiichiHand,
        RiichiRules, RiichiVariant, Seat, TableProgress, TileId, WallSeed,
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
        pass_all_with(hand, discarder, &RejectAllHandJudge)
    }

    fn pass_all_with(
        hand: &mut RiichiHand,
        discarder: Seat,
        judge: &dyn HandJudge,
    ) -> crate::HandTransition {
        let variant = hand.rules().variant;
        let mut last = None;
        for offset in 1..variant.seat_count().value() {
            let seat = super::seat_at_offset(variant, discarder, offset);
            last = Some(hand.pass(seat, judge).expect("pass"));
        }
        last.expect("a riichi hand always has opponents")
    }

    struct SeatOneTenpai;

    impl HandJudge for SeatOneTenpai {
        fn is_tenpai(&self, _rules: &RiichiRules, _player: &PlayerHand, seat: Seat) -> bool {
            seat.index() == 1
        }
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
        assert!(hand.pass(dealer, &RejectAllHandJudge).is_err());
        assert_eq!(hand, before_self_pass);

        hand.pass(responder, &RejectAllHandJudge)
            .expect("first pass");
        let after_first_pass = hand.clone();
        assert!(hand.pass(responder, &RejectAllHandJudge).is_err());
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
                    reason: EndReason::ExhaustiveDraw,
                    ..
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

    #[test]
    fn exhaustive_draw_records_judge_tenpai_seats() {
        let mut hand = start_hand(RiichiVariant::Sanma, 0);
        let final_transition = loop {
            let HandPhase::AwaitingTurnAction { seat } = hand.phase() else {
                panic!("expected turn");
            };
            let drawn = hand
                .player(seat)
                .expect("player")
                .drawn_tile_id()
                .expect("draw");
            hand.discard(seat, drawn).expect("discard");
            let transition = pass_all_with(&mut hand, seat, &SeatOneTenpai);
            if matches!(
                transition.events().last(),
                Some(HandEvent::ExhaustiveDrawDeclared { .. })
            ) {
                break transition;
            }
        };

        assert!(matches!(
            final_transition.events().last(),
            Some(HandEvent::ExhaustiveDrawDeclared {
                reason: EndReason::ExhaustiveDraw,
                tenpai,
            }) if tenpai.as_ref() == [Seat::new(RiichiVariant::Sanma, 1).expect("seat")]
        ));
    }
}
