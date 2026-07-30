use crate::{
    DrawSource, HandError, HandEvent, HandTransition, Meld, MeldId, MeldKind, Rank, Reaction,
    ReactionKind, RiichiHand, RiichiStatus, RiichiVariant, Seat, Tile, TileId, TileKind,
};

use super::state::{PendingDiscard, PendingKan, Phase};

impl RiichiHand {
    pub fn pass(&mut self, actor: Seat) -> Result<HandTransition, HandError> {
        self.respond(actor, Reaction::Pass)
    }

    pub fn respond(
        &mut self,
        actor: Seat,
        reaction: Reaction,
    ) -> Result<HandTransition, HandError> {
        self.validate_seat(actor)?;
        match &self.phase {
            Phase::Responses(pending) => {
                self.validate_discard_reaction(pending, actor, &reaction)?;
            }
            Phase::KanResponses(pending) => {
                if actor == pending.declarer() {
                    return Err(HandError::DeclarerCannotReact);
                }
                if pending.responses()[usize::from(actor.index())].is_some() {
                    return Err(HandError::AlreadyResponded { seat: actor });
                }
                if !matches!(reaction, Reaction::Pass) {
                    return Err(HandError::InvalidReaction {
                        reason: "only pass is available until the win judge accepts kan ron",
                    });
                }
            }
            _ => return Err(HandError::WrongPhase),
        }

        let reaction_kind = reaction_kind(&reaction);
        let all_responded = match &mut self.phase {
            Phase::Responses(pending) => {
                pending.responses[usize::from(actor.index())] = Some(reaction);
                pending.responses.iter().all(Option::is_some)
            }
            Phase::KanResponses(pending) => {
                pending.responses_mut()[usize::from(actor.index())] = Some(reaction);
                pending.responses().iter().all(Option::is_some)
            }
            _ => unreachable!("phase was validated above"),
        };

        let mut transition = HandTransition::new(vec![HandEvent::ReactionSubmitted {
            seat: actor,
            reaction: reaction_kind,
        }]);
        if all_responded {
            let resolution = match &self.phase {
                Phase::Responses(pending) => {
                    let pending = pending.clone();
                    self.resolve_discard_responses(&pending)
                }
                Phase::KanResponses(pending) => {
                    let pending = pending.clone();
                    self.complete_pending_kan(pending)
                }
                _ => unreachable!("phase was validated above"),
            };
            transition.append(resolution);
        }
        Ok(transition)
    }

    pub fn declare_concealed_kan(
        &mut self,
        actor: Seat,
        tile_ids: [TileId; 4],
    ) -> Result<HandTransition, HandError> {
        self.validate_turn_kan(actor)?;
        let tiles = selected_tiles(
            &self.players[usize::from(actor.index())].concealed,
            &tile_ids,
        )?;
        if !tiles.iter().all(|tile| tile.kind() == tiles[0].kind()) {
            return Err(HandError::InvalidReaction {
                reason: "concealed kan requires four identical tile kinds",
            });
        }

        let mut responses = self.empty_responses(actor);
        responses[usize::from(actor.index())] = Some(Reaction::Pass);
        self.phase = Phase::KanResponses(PendingKan::Concealed {
            declarer: actor,
            tile_ids,
            responses,
        });
        Ok(HandTransition::new(vec![HandEvent::KanProposed {
            seat: actor,
            kind: MeldKind::ConcealedKan,
            tile_kind: tiles[0].kind(),
        }]))
    }

    pub fn declare_added_kan(
        &mut self,
        actor: Seat,
        meld_id: MeldId,
        tile_id: TileId,
    ) -> Result<HandTransition, HandError> {
        self.validate_turn_kan(actor)?;
        let player = &self.players[usize::from(actor.index())];
        let tile = player
            .concealed
            .iter()
            .find(|tile| tile.id() == tile_id)
            .copied()
            .ok_or(HandError::TileNotInHand { tile_id })?;
        let meld = player
            .melds
            .iter()
            .find(|meld| meld.id() == meld_id)
            .ok_or(HandError::MeldNotFound { meld_id })?;
        if !matches!(meld.kind(), MeldKind::Pon) {
            return Err(HandError::MeldCannotBeAddedKan { meld_id });
        }
        if meld.tile_kind() != tile.kind() {
            return Err(HandError::InvalidReaction {
                reason: "added tile must match the pon tile kind",
            });
        }

        let mut responses = self.empty_responses(actor);
        responses[usize::from(actor.index())] = Some(Reaction::Pass);
        self.phase = Phase::KanResponses(PendingKan::Added {
            declarer: actor,
            meld_id,
            tile_id,
            responses,
        });
        Ok(HandTransition::new(vec![HandEvent::KanProposed {
            seat: actor,
            kind: MeldKind::AddedKan,
            tile_kind: tile.kind(),
        }]))
    }

    fn validate_discard_reaction(
        &self,
        pending: &PendingDiscard,
        actor: Seat,
        reaction: &Reaction,
    ) -> Result<(), HandError> {
        if actor == pending.discarder {
            return Err(HandError::DiscarderCannotReact);
        }
        if pending.responses[usize::from(actor.index())].is_some() {
            return Err(HandError::AlreadyResponded { seat: actor });
        }
        if matches!(reaction, Reaction::Pass) {
            return Ok(());
        }
        if self.wall.remaining_live_draws() == 0 {
            return Err(HandError::CannotCallOnLastDiscard);
        }

        let player = &self.players[usize::from(actor.index())];
        if !matches!(player.riichi, RiichiStatus::None) {
            return Err(HandError::CannotCallWhileRiichi);
        }
        let called_tile = self.players[usize::from(pending.discarder.index())].discards
            [pending.discard_index]
            .tile();

        match reaction {
            Reaction::Pass => Ok(()),
            Reaction::Pon { hand_tiles } => {
                validate_matching_call(&player.concealed, hand_tiles, called_tile, 2)
            }
            Reaction::OpenKan { hand_tiles } => {
                self.validate_kan_capacity()?;
                validate_matching_call(&player.concealed, hand_tiles, called_tile, 3)
            }
            Reaction::Chi { hand_tiles } => {
                if matches!(self.rules.variant, RiichiVariant::Sanma) {
                    return Err(HandError::InvalidReaction {
                        reason: "chi is unavailable in sanma",
                    });
                }
                let next = super::state::seat_at_offset(self.rules.variant, pending.discarder, 1);
                if actor != next {
                    return Err(HandError::InvalidReaction {
                        reason: "only the next seat can chi",
                    });
                }
                validate_chi(&player.concealed, hand_tiles, called_tile)
            }
        }
    }

    fn resolve_discard_responses(&mut self, pending: &PendingDiscard) -> HandTransition {
        let selected = (1..self.rules.variant.seat_count().value())
            .map(|offset| {
                let seat =
                    super::state::seat_at_offset(self.rules.variant, pending.discarder, offset);
                (seat, &pending.responses[usize::from(seat.index())])
            })
            .find_map(|(seat, reaction)| match reaction {
                Some(reaction @ (Reaction::Pon { .. } | Reaction::OpenKan { .. })) => {
                    Some((seat, reaction.clone()))
                }
                _ => None,
            })
            .or_else(|| {
                let seat = super::state::seat_at_offset(self.rules.variant, pending.discarder, 1);
                match &pending.responses[usize::from(seat.index())] {
                    Some(reaction @ Reaction::Chi { .. }) => Some((seat, reaction.clone())),
                    _ => None,
                }
            });

        let Some((caller, reaction)) = selected else {
            return self.resolve_unclaimed_discard(pending);
        };
        self.apply_discard_call(pending, caller, reaction)
    }

    fn apply_discard_call(
        &mut self,
        pending: &PendingDiscard,
        caller: Seat,
        reaction: Reaction,
    ) -> HandTransition {
        let called_tile = self.players[usize::from(pending.discarder.index())].discards
            [pending.discard_index]
            .tile();
        let (kind, hand_tile_ids): (MeldKind, Vec<TileId>) = match reaction {
            Reaction::Chi { hand_tiles } => (MeldKind::Chi, hand_tiles.into()),
            Reaction::Pon { hand_tiles } => (MeldKind::Pon, hand_tiles.into()),
            Reaction::OpenKan { hand_tiles } => (MeldKind::OpenKan, hand_tiles.into()),
            Reaction::Pass => unreachable!("pass cannot be selected as a call"),
        };
        let mut tiles = remove_tiles(
            &mut self.players[usize::from(caller.index())].concealed,
            &hand_tile_ids,
        );
        tiles.push(called_tile);
        tiles.sort_unstable_by_key(|tile| (tile.kind().index(), tile.id()));

        let meld_id = next_meld_id(&self.players[usize::from(caller.index())]);
        let meld = Meld::new(
            meld_id,
            kind,
            tiles,
            Some(pending.discarder),
            Some(called_tile.id()),
        );
        self.players[usize::from(caller.index())]
            .melds
            .push(meld.clone());
        self.players[usize::from(pending.discarder.index())].discards[pending.discard_index]
            .mark_claimed(caller);
        self.calls_occurred = true;
        cancel_ippatsu(&mut self.players);

        let mut transition = HandTransition::new(vec![HandEvent::MeldDeclared {
            seat: caller,
            meld: meld.clone(),
        }]);
        if matches!(kind, MeldKind::OpenKan) {
            transition.append(self.finish_kan(caller, meld));
        } else {
            let forbidden_discards = forbidden_after_call(kind, called_tile, &meld);
            self.players[usize::from(caller.index())].drawn_tile = None;
            self.phase = Phase::DiscardAfterCall {
                seat: caller,
                forbidden_discards,
            };
        }
        transition
    }

    fn validate_turn_kan(&self, actor: Seat) -> Result<(), HandError> {
        self.validate_seat(actor)?;
        let Phase::TurnAction { seat } = self.phase else {
            return Err(HandError::WrongPhase);
        };
        if actor != seat {
            return Err(HandError::NotActiveSeat {
                expected: seat,
                actual: actor,
            });
        }
        self.validate_kan_capacity()
    }

    fn validate_kan_capacity(&self) -> Result<(), HandError> {
        if self.wall.remaining_live_draws() == 0 {
            return Err(HandError::KanNotAllowedOnLastDraw);
        }
        if self.kan_counts.iter().copied().sum::<u8>() >= 4 {
            return Err(HandError::KanLimitReached);
        }
        Ok(())
    }

    fn empty_responses(&self, _declarer: Seat) -> Box<[Option<Reaction>]> {
        vec![None; usize::from(self.rules.variant.seat_count().value())].into_boxed_slice()
    }

    fn complete_pending_kan(&mut self, pending: PendingKan) -> HandTransition {
        match pending {
            PendingKan::Concealed {
                declarer, tile_ids, ..
            } => {
                let mut tiles = remove_tiles(
                    &mut self.players[usize::from(declarer.index())].concealed,
                    &tile_ids,
                );
                tiles.sort_unstable_by_key(|tile| tile.id());
                let meld = Meld::new(
                    next_meld_id(&self.players[usize::from(declarer.index())]),
                    MeldKind::ConcealedKan,
                    tiles,
                    None,
                    None,
                );
                self.players[usize::from(declarer.index())]
                    .melds
                    .push(meld.clone());
                self.finish_kan(declarer, meld)
            }
            PendingKan::Added {
                declarer,
                meld_id,
                tile_id,
                ..
            } => {
                let tile = remove_tiles(
                    &mut self.players[usize::from(declarer.index())].concealed,
                    &[tile_id],
                )
                .pop()
                .expect("validated added-kan tile exists");
                let completed_meld = {
                    let meld = self.players[usize::from(declarer.index())]
                        .melds
                        .iter_mut()
                        .find(|meld| meld.id() == meld_id)
                        .expect("validated pon still exists");
                    let mut tiles = std::mem::take(&mut meld.tiles).into_vec();
                    tiles.push(tile);
                    tiles.sort_unstable_by_key(|tile| tile.id());
                    meld.tiles = tiles.into_boxed_slice();
                    meld.kind = MeldKind::AddedKan;
                    meld.clone()
                };
                self.finish_kan(declarer, completed_meld)
            }
        }
    }

    fn finish_kan(&mut self, actor: Seat, meld: Meld) -> HandTransition {
        self.kan_counts[usize::from(actor.index())] += 1;
        self.calls_occurred = true;
        cancel_ippatsu(&mut self.players);

        let mut events = vec![HandEvent::KanCompleted { seat: actor, meld }];
        if self.rules.bonuses.kan_dora {
            let indicator = self
                .wall
                .reveal_next_dora()
                .expect("at most four kans can reveal at most five indicators");
            events.push(HandEvent::DoraIndicatorRevealed {
                tile: indicator,
                revealed_count: self.wall.revealed_dora_count(),
            });
        }
        let tile = self
            .wall
            .draw_rinshan()
            .expect("validated kan capacity guarantees a rinshan draw");
        let player = &mut self.players[usize::from(actor.index())];
        player.concealed.push(tile);
        player.drawn_tile = Some(tile.id());
        player.temporary_furiten = false;
        self.phase = Phase::TurnAction { seat: actor };
        events.push(HandEvent::TileDrawn {
            seat: actor,
            tile,
            source: DrawSource::Rinshan,
            remaining_live_draws: self.wall.remaining_live_draws(),
        });
        HandTransition::new(events)
    }
}

fn reaction_kind(reaction: &Reaction) -> ReactionKind {
    match reaction {
        Reaction::Pass => ReactionKind::Pass,
        Reaction::Chi { .. } => ReactionKind::Chi,
        Reaction::Pon { .. } => ReactionKind::Pon,
        Reaction::OpenKan { .. } => ReactionKind::OpenKan,
    }
}

fn validate_matching_call<const N: usize>(
    concealed: &[Tile],
    tile_ids: &[TileId; N],
    called_tile: Tile,
    expected_count: usize,
) -> Result<(), HandError> {
    debug_assert_eq!(N, expected_count);
    let tiles = selected_tiles(concealed, tile_ids)?;
    if tiles.iter().all(|tile| tile.kind() == called_tile.kind()) {
        Ok(())
    } else {
        Err(HandError::InvalidReaction {
            reason: "selected tiles must match the called tile kind",
        })
    }
}

fn validate_chi(
    concealed: &[Tile],
    tile_ids: &[TileId; 2],
    called_tile: Tile,
) -> Result<(), HandError> {
    let Some(called_suit) = called_tile.kind().suit() else {
        return Err(HandError::InvalidReaction {
            reason: "honor tiles cannot be used in chi",
        });
    };
    let selected = selected_tiles(concealed, tile_ids)?;
    let mut ranks = vec![
        called_tile
            .kind()
            .rank()
            .expect("suited tile has rank")
            .value(),
    ];
    for tile in selected {
        if tile.kind().suit() != Some(called_suit) {
            return Err(HandError::InvalidReaction {
                reason: "chi tiles must have the same suit",
            });
        }
        ranks.push(tile.kind().rank().expect("suited tile has rank").value());
    }
    ranks.sort_unstable();
    if ranks[1] == ranks[0] + 1 && ranks[2] == ranks[1] + 1 {
        Ok(())
    } else {
        Err(HandError::InvalidReaction {
            reason: "chi tiles must form a consecutive sequence",
        })
    }
}

fn selected_tiles<const N: usize>(
    concealed: &[Tile],
    tile_ids: &[TileId; N],
) -> Result<Vec<Tile>, HandError> {
    for left in 0..N {
        for right in left + 1..N {
            if tile_ids[left] == tile_ids[right] {
                return Err(HandError::DuplicateTileSelection);
            }
        }
    }
    tile_ids
        .iter()
        .map(|tile_id| {
            concealed
                .iter()
                .find(|tile| tile.id() == *tile_id)
                .copied()
                .ok_or(HandError::TileNotInHand { tile_id: *tile_id })
        })
        .collect()
}

fn remove_tiles(concealed: &mut Vec<Tile>, tile_ids: &[TileId]) -> Vec<Tile> {
    tile_ids
        .iter()
        .map(|tile_id| {
            let index = concealed
                .iter()
                .position(|tile| tile.id() == *tile_id)
                .expect("selected tiles were validated before mutation");
            concealed.swap_remove(index)
        })
        .collect()
}

fn next_meld_id(player: &crate::PlayerHand) -> MeldId {
    MeldId::new(u8::try_from(player.melds.len()).expect("a player can have at most four melds"))
}

fn forbidden_after_call(kind: MeldKind, called_tile: Tile, meld: &Meld) -> Box<[TileKind]> {
    let mut forbidden = vec![called_tile.kind()];
    if matches!(kind, MeldKind::Chi) {
        let called_rank = called_tile
            .kind()
            .rank()
            .expect("chi called tile is suited")
            .value();
        let mut ranks: Vec<_> = meld
            .tiles()
            .iter()
            .map(|tile| tile.kind().rank().expect("chi tile is suited").value())
            .collect();
        ranks.sort_unstable();
        let replacement_rank = if called_rank == ranks[0] {
            called_rank.checked_add(3).filter(|rank| *rank <= Rank::MAX)
        } else if called_rank == ranks[2] {
            called_rank.checked_sub(3).filter(|rank| *rank >= Rank::MIN)
        } else {
            None
        };
        if let (Some(suit), Some(rank)) = (called_tile.kind().suit(), replacement_rank) {
            forbidden.push(TileKind::suited(
                suit,
                Rank::new(rank).expect("replacement rank is in range"),
            ));
        }
    }
    forbidden.sort_unstable();
    forbidden.dedup();
    forbidden.into_boxed_slice()
}

fn cancel_ippatsu(players: &mut [crate::PlayerHand]) {
    for player in players {
        player.ippatsu_eligible = false;
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        DrawSource, HandEvent, HandPhase, MeldKind, Reaction, RiichiHand, RiichiRules,
        RiichiVariant, Seat, TableProgress, TileId, TileKind, WallSeed,
    };

    use super::{Phase, forbidden_after_call, validate_chi};

    fn start_yonma(seed_number: u32) -> RiichiHand {
        let dealer = Seat::new(RiichiVariant::Yonma, 0).expect("dealer");
        let progress = TableProgress::east_one(RiichiVariant::Yonma, dealer).expect("progress");
        let mut seed = [0_u8; 32];
        seed[..4].copy_from_slice(&seed_number.to_le_bytes());
        RiichiHand::start(
            RiichiRules::default(),
            progress,
            [25_000; 4],
            &WallSeed::from_bytes(seed),
        )
        .expect("start")
        .0
    }

    fn find_matching_scenario(required: usize) -> (RiichiHand, Seat, TileId, Vec<TileId>) {
        let dealer = Seat::new(RiichiVariant::Yonma, 0).expect("dealer");
        for seed in 0..10_000 {
            let hand = start_yonma(seed);
            let dealer_tiles = hand.player(dealer).expect("dealer").concealed().to_vec();
            for discard in dealer_tiles {
                for caller_index in 1..4 {
                    let caller = Seat::new(RiichiVariant::Yonma, caller_index).expect("caller");
                    let matching: Vec<_> = hand
                        .player(caller)
                        .expect("caller hand")
                        .concealed()
                        .iter()
                        .filter(|tile| tile.kind() == discard.kind())
                        .map(|tile| tile.id())
                        .collect();
                    if matching.len() >= required {
                        return (hand, caller, discard.id(), matching);
                    }
                }
            }
        }
        panic!("deterministic seed search did not find a matching call");
    }

    fn find_chi_scenario() -> (RiichiHand, Seat, TileId, [TileId; 2], TileId) {
        let dealer = Seat::new(RiichiVariant::Yonma, 0).expect("dealer");
        let caller = Seat::new(RiichiVariant::Yonma, 1).expect("next seat");
        for seed in 0..10_000 {
            let hand = start_yonma(seed);
            let dealer_tiles = hand.player(dealer).expect("dealer").concealed().to_vec();
            let caller_tiles = hand.player(caller).expect("caller").concealed().to_vec();
            for discard in dealer_tiles {
                for left in 0..caller_tiles.len() {
                    for right in left + 1..caller_tiles.len() {
                        let selected = [caller_tiles[left].id(), caller_tiles[right].id()];
                        if validate_chi(&caller_tiles, &selected, discard).is_err() {
                            continue;
                        }
                        let mut meld_tiles = vec![caller_tiles[left], caller_tiles[right], discard];
                        meld_tiles.sort_unstable_by_key(|tile| tile.kind().index());
                        let meld = crate::Meld::new(
                            crate::MeldId::new(0),
                            MeldKind::Chi,
                            meld_tiles,
                            Some(dealer),
                            Some(discard.id()),
                        );
                        let forbidden = forbidden_after_call(MeldKind::Chi, discard, &meld);
                        if let Some(forbidden_tile) = caller_tiles.iter().find(|tile| {
                            tile.id() != selected[0]
                                && tile.id() != selected[1]
                                && forbidden.contains(&tile.kind())
                        }) {
                            return (hand, caller, discard.id(), selected, forbidden_tile.id());
                        }
                    }
                }
            }
        }
        panic!("deterministic seed search did not find a chi with replacement restriction");
    }

    fn find_priority_scenario() -> (RiichiHand, TileId, [TileId; 2], Seat, [TileId; 2]) {
        let dealer = Seat::new(RiichiVariant::Yonma, 0).expect("dealer");
        let chi_seat = Seat::new(RiichiVariant::Yonma, 1).expect("next seat");
        for seed in 0..20_000 {
            let hand = start_yonma(seed);
            let dealer_tiles = hand.player(dealer).expect("dealer").concealed().to_vec();
            let chi_tiles = hand
                .player(chi_seat)
                .expect("chi player")
                .concealed()
                .to_vec();
            for discard in dealer_tiles {
                let chi_selection = (0..chi_tiles.len()).find_map(|left| {
                    (left + 1..chi_tiles.len()).find_map(|right| {
                        let selection = [chi_tiles[left].id(), chi_tiles[right].id()];
                        validate_chi(&chi_tiles, &selection, discard)
                            .is_ok()
                            .then_some(selection)
                    })
                });
                let Some(chi_selection) = chi_selection else {
                    continue;
                };
                for pon_index in 2..4 {
                    let pon_seat = Seat::new(RiichiVariant::Yonma, pon_index).expect("pon seat");
                    let matching: Vec<_> = hand
                        .player(pon_seat)
                        .expect("pon player")
                        .concealed()
                        .iter()
                        .filter(|tile| tile.kind() == discard.kind())
                        .map(|tile| tile.id())
                        .collect();
                    if matching.len() >= 2 {
                        return (
                            hand,
                            discard.id(),
                            chi_selection,
                            pon_seat,
                            [matching[0], matching[1]],
                        );
                    }
                }
            }
        }
        panic!("deterministic seed search did not find a chi/pon priority case");
    }

    fn pass_remaining(
        hand: &mut RiichiHand,
        trigger: Seat,
        already_responded: &[Seat],
    ) -> crate::HandTransition {
        let mut last = None;
        for offset in 1..hand.rules().variant.seat_count().value() {
            let seat = super::super::state::seat_at_offset(hand.rules().variant, trigger, offset);
            if !already_responded.contains(&seat) {
                last = Some(hand.pass(seat).expect("pass"));
            }
        }
        last.expect("at least one response remains")
    }

    #[test]
    fn pon_claims_discard_and_transfers_turn_without_draw() {
        let (mut hand, caller, discard_id, matching) = find_matching_scenario(2);
        let dealer = Seat::new(RiichiVariant::Yonma, 0).expect("dealer");
        hand.discard(dealer, discard_id).expect("discard");
        hand.respond(
            caller,
            Reaction::Pon {
                hand_tiles: [matching[0], matching[1]],
            },
        )
        .expect("pon");

        let transition = pass_remaining(&mut hand, dealer, &[caller]);

        assert_eq!(hand.phase(), HandPhase::AwaitingDiscard { seat: caller });
        assert_eq!(hand.player(caller).expect("caller").melds().len(), 1);
        assert_eq!(
            hand.player(caller).expect("caller").melds()[0].kind(),
            MeldKind::Pon
        );
        assert!(
            transition.events().iter().any(
                |event| matches!(event, HandEvent::MeldDeclared { seat, .. } if *seat == caller)
            )
        );
    }

    #[test]
    fn chi_is_next_seat_only_and_enforces_replacement_restriction() {
        let (mut hand, caller, discard_id, selected, forbidden_discard) = find_chi_scenario();
        let dealer = Seat::new(RiichiVariant::Yonma, 0).expect("dealer");
        hand.discard(dealer, discard_id).expect("discard");
        hand.respond(
            caller,
            Reaction::Chi {
                hand_tiles: selected,
            },
        )
        .expect("chi");
        pass_remaining(&mut hand, dealer, &[caller]);

        assert_eq!(
            hand.player(caller).expect("caller").melds()[0].kind(),
            MeldKind::Chi
        );
        let before = hand.clone();
        assert!(hand.discard(caller, forbidden_discard).is_err());
        assert_eq!(hand, before);
    }

    #[test]
    fn pon_beats_chi_regardless_of_response_arrival_order() {
        let (mut hand, discard_id, chi_tiles, pon_seat, pon_tiles) = find_priority_scenario();
        let dealer = Seat::new(RiichiVariant::Yonma, 0).expect("dealer");
        let chi_seat = Seat::new(RiichiVariant::Yonma, 1).expect("chi seat");
        hand.discard(dealer, discard_id).expect("discard");
        hand.respond(
            chi_seat,
            Reaction::Chi {
                hand_tiles: chi_tiles,
            },
        )
        .expect("chi response arrives first");
        hand.respond(
            pon_seat,
            Reaction::Pon {
                hand_tiles: pon_tiles,
            },
        )
        .expect("pon response arrives second");
        pass_remaining(&mut hand, dealer, &[chi_seat, pon_seat]);

        assert!(
            hand.player(chi_seat)
                .expect("chi player")
                .melds()
                .is_empty()
        );
        assert_eq!(
            hand.player(pon_seat).expect("pon player").melds()[0].kind(),
            MeldKind::Pon
        );
        assert_eq!(hand.phase(), HandPhase::AwaitingDiscard { seat: pon_seat });
    }

    #[test]
    fn open_kan_reveals_dora_and_draws_rinshan() {
        let (mut hand, caller, discard_id, matching) = find_matching_scenario(3);
        let dealer = Seat::new(RiichiVariant::Yonma, 0).expect("dealer");
        let before = hand.remaining_live_draws();
        hand.discard(dealer, discard_id).expect("discard");
        hand.respond(
            caller,
            Reaction::OpenKan {
                hand_tiles: [matching[0], matching[1], matching[2]],
            },
        )
        .expect("kan");

        let transition = pass_remaining(&mut hand, dealer, &[caller]);

        assert_eq!(hand.phase(), HandPhase::AwaitingTurnAction { seat: caller });
        assert_eq!(hand.remaining_live_draws(), before - 1);
        assert!(transition.events().iter().any(|event| matches!(
            event,
            HandEvent::DoraIndicatorRevealed {
                revealed_count: 2,
                ..
            }
        )));
        assert!(transition.events().iter().any(|event| matches!(
            event,
            HandEvent::TileDrawn {
                seat,
                source: DrawSource::Rinshan,
                ..
            } if *seat == caller
        )));
    }

    #[test]
    fn concealed_and_added_kan_wait_for_responses_before_mutation() {
        let (mut concealed_hand, actor, dealer_tile_id, matching) = find_matching_scenario(3);
        let dealer = Seat::new(RiichiVariant::Yonma, 0).expect("dealer");
        let dealer_index = concealed_hand.players[usize::from(dealer.index())]
            .concealed
            .iter()
            .position(|tile| tile.id() == dealer_tile_id)
            .expect("dealer tile");
        let fourth = concealed_hand.players[usize::from(dealer.index())]
            .concealed
            .swap_remove(dealer_index);
        concealed_hand.players[usize::from(actor.index())]
            .concealed
            .push(fourth);
        concealed_hand.players[usize::from(actor.index())].drawn_tile = Some(fourth.id());
        concealed_hand.phase = Phase::TurnAction { seat: actor };

        concealed_hand
            .declare_concealed_kan(actor, [matching[0], matching[1], matching[2], fourth.id()])
            .expect("propose concealed kan");
        assert!(
            concealed_hand
                .player(actor)
                .expect("actor")
                .melds()
                .is_empty()
        );
        pass_remaining(&mut concealed_hand, actor, &[]);
        assert_eq!(
            concealed_hand.player(actor).expect("actor").melds()[0].kind(),
            MeldKind::ConcealedKan
        );

        let (mut added_hand, caller, discard_id, matching) = find_matching_scenario(3);
        added_hand.discard(dealer, discard_id).expect("discard");
        added_hand
            .respond(
                caller,
                Reaction::Pon {
                    hand_tiles: [matching[0], matching[1]],
                },
            )
            .expect("pon");
        pass_remaining(&mut added_hand, dealer, &[caller]);
        added_hand.phase = Phase::TurnAction { seat: caller };
        added_hand.players[usize::from(caller.index())].drawn_tile = Some(matching[2]);

        added_hand
            .declare_added_kan(caller, crate::MeldId::new(0), matching[2])
            .expect("propose added kan");
        assert_eq!(
            added_hand.player(caller).expect("caller").melds()[0].kind(),
            MeldKind::Pon
        );
        pass_remaining(&mut added_hand, caller, &[]);
        assert_eq!(
            added_hand.player(caller).expect("caller").melds()[0].kind(),
            MeldKind::AddedKan
        );
    }

    #[test]
    fn invalid_call_is_atomic() {
        let (mut hand, caller, discard_id, _) = find_matching_scenario(2);
        let dealer = Seat::new(RiichiVariant::Yonma, 0).expect("dealer");
        hand.discard(dealer, discard_id).expect("discard");
        let before = hand.clone();

        assert!(
            hand.respond(
                caller,
                Reaction::Pon {
                    hand_tiles: [TileId::new(u16::MAX), TileId::new(u16::MAX - 1)]
                }
            )
            .is_err()
        );
        assert_eq!(hand, before);
    }

    #[test]
    fn call_helpers_reject_impossible_empty_inputs() {
        let called = crate::Tile::new(
            crate::TileId::new(99),
            "3p".parse::<TileKind>().expect("kind"),
            false,
        )
        .expect("tile");
        assert!(
            validate_chi(&[], &[crate::TileId::new(1), crate::TileId::new(2)], called).is_err()
        );

        let meld = crate::Meld::new(
            crate::MeldId::new(0),
            MeldKind::Chi,
            [
                crate::Tile::new(crate::TileId::new(1), "3p".parse().expect("kind"), false)
                    .expect("tile"),
                crate::Tile::new(crate::TileId::new(2), "4p".parse().expect("kind"), false)
                    .expect("tile"),
                crate::Tile::new(crate::TileId::new(3), "5p".parse().expect("kind"), false)
                    .expect("tile"),
            ],
            Some(Seat::new(RiichiVariant::Yonma, 0).expect("seat")),
            Some(called.id()),
        );
        let forbidden = forbidden_after_call(MeldKind::Chi, called, &meld);
        assert!(forbidden.contains(&"3p".parse().expect("kind")));
        assert!(forbidden.contains(&"6p".parse().expect("kind")));
    }
}
