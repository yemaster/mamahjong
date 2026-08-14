use crate::{
    DrawSource, EndReason, HandError, HandEvent, HandJudge, HandTransition, KanQuery, KuikaeRule,
    Meld, MeldId, MeldKind, Rank, Reaction, ReactionKind, RiichiHand, RiichiScorer, RiichiStatus,
    RiichiVariant, RonResolution, Seat, Tile, TileId, TileKind, WinEvaluation, WinQuery, WinSource,
};

use super::state::{PendingDiscard, PendingKan, Phase};

impl RiichiHand {
    pub fn available_reactions(
        &self,
        actor: Seat,
        judge: &dyn HandJudge,
    ) -> Result<Vec<Reaction>, HandError> {
        self.validate_seat(actor)?;
        match &self.phase {
            Phase::Responses(pending) => {
                if actor == pending.discarder
                    || pending.responses[usize::from(actor.index())].is_some()
                {
                    return Ok(Vec::new());
                }
                Ok(self.discard_reaction_options(pending, actor, judge))
            }
            Phase::KanResponses(pending) => {
                if actor == pending.declarer()
                    || pending.responses()[usize::from(actor.index())].is_some()
                {
                    return Ok(Vec::new());
                }
                Ok(self
                    .can_ron(actor, judge)
                    .is_some()
                    .then_some(Reaction::Ron)
                    .into_iter()
                    .collect())
            }
            _ => Ok(Vec::new()),
        }
    }

    pub fn advance_automatic_reactions(
        &mut self,
        judge: &dyn HandJudge,
    ) -> Result<HandTransition, HandError> {
        let unavailable = (0..self.rules.variant.seat_count().value())
            .filter_map(|index| {
                let seat =
                    Seat::new(self.rules.variant, index).expect("seat index is within variant");
                match self.available_reactions(seat, judge) {
                    Ok(options) if options.is_empty() => Some(Ok(seat)),
                    Ok(_) => None,
                    Err(error) => Some(Err(error)),
                }
            })
            .collect::<Result<Vec<_>, _>>()?;

        let early_resolve = match &self.phase {
            Phase::Responses(pending) => {
                !pending.responses.iter().all(Option::is_some)
                    && self.can_resolve_discard_early(pending, judge)
            }
            _ => false,
        };

        match &mut self.phase {
            Phase::Responses(pending) => {
                for seat in unavailable {
                    let response = &mut pending.responses[usize::from(seat.index())];
                    if response.is_none() {
                        *response = Some(Reaction::Pass);
                    }
                }
                if pending.responses.iter().all(Option::is_some) {
                    let pending = pending.clone();
                    return Ok(self.resolve_discard_responses(&pending, judge));
                }
                if early_resolve {
                    for response in pending.responses.iter_mut() {
                        if response.is_none() {
                            *response = Some(Reaction::Pass);
                        }
                    }
                    let pending = pending.clone();
                    return Ok(self.resolve_discard_responses(&pending, judge));
                }
                Ok(HandTransition::default())
            }
            Phase::KanResponses(pending) => {
                for seat in unavailable {
                    let response = &mut pending.responses_mut()[usize::from(seat.index())];
                    if response.is_none() {
                        *response = Some(Reaction::Pass);
                    }
                }
                if pending.responses().iter().all(Option::is_some) {
                    let pending = pending.clone();
                    Ok(self.resolve_kan_responses(pending))
                } else {
                    Ok(HandTransition::default())
                }
            }
            _ => Ok(HandTransition::default()),
        }
    }

    fn can_resolve_discard_early(&self, pending: &PendingDiscard, judge: &dyn HandJudge) -> bool {
        let best_submitted = pending
            .responses
            .iter()
            .enumerate()
            .filter_map(|(index, response)| match response {
                Some(reaction) if !matches!(reaction, Reaction::Pass) => {
                    let seat = Seat::new(self.rules.variant, u8::try_from(index).ok()?)
                        .expect("valid seat");
                    Some((seat, reaction_priority(reaction)))
                }
                _ => None,
            })
            .max_by_key(|(_, priority)| *priority);

        let Some((_, best_priority)) = best_submitted else {
            return false;
        };

        for index in 0..self.rules.variant.seat_count().value() {
            if pending.responses[usize::from(index)].is_some() {
                continue;
            }
            let seat = Seat::new(self.rules.variant, index).expect("valid seat");
            if let Ok(options) = self.available_reactions(seat, judge) {
                for option in options {
                    if reaction_priority(&option) >= best_priority {
                        return false;
                    }
                }
            }
        }
        true
    }

    pub fn evaluate_pending_ron(&self, actor: Seat) -> Result<WinEvaluation, HandError> {
        self.validate_seat(actor)?;
        let (tile, source) = self
            .can_ron(actor, &RiichiScorer)
            .ok_or(HandError::WinNotAllowed)?;
        let player = &self.players[usize::from(actor.index())];
        RiichiScorer
            .evaluate(WinQuery::new(
                &self.rules,
                self.progress,
                actor,
                player,
                tile,
                source,
                &self.wall,
                self.calls_occurred,
            ))
            .ok_or(HandError::WinNotAllowed)
    }

    pub fn pass(
        &mut self,
        actor: Seat,
        judge: &dyn HandJudge,
    ) -> Result<HandTransition, HandError> {
        self.respond(actor, Reaction::Pass, judge)
    }

    pub fn respond(
        &mut self,
        actor: Seat,
        reaction: Reaction,
        judge: &dyn HandJudge,
    ) -> Result<HandTransition, HandError> {
        self.validate_seat(actor)?;
        match &self.phase {
            Phase::Responses(pending) => {
                self.validate_discard_reaction(pending, actor, &reaction, judge)?;
            }
            Phase::KanResponses(pending) => {
                self.validate_kan_reaction(pending, actor, &reaction, judge)?;
            }
            _ => return Err(HandError::WrongPhase),
        }

        let reaction_kind = reaction_kind(&reaction);
        let declined_ron =
            matches!(reaction, Reaction::Pass) && self.can_ron(actor, judge).is_some();
        let furiten_event = declined_ron.then(|| self.apply_declined_ron(actor));
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
        if let Some(event) = furiten_event {
            transition.append(HandTransition::new(vec![event]));
        }
        if all_responded {
            let resolution = match &self.phase {
                Phase::Responses(pending) => {
                    let pending = pending.clone();
                    self.resolve_discard_responses(&pending, judge)
                }
                Phase::KanResponses(pending) => {
                    let pending = pending.clone();
                    self.resolve_kan_responses(pending)
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
        judge: &dyn HandJudge,
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
        let player = &self.players[usize::from(actor.index())];
        if matches!(player.riichi, RiichiStatus::Established)
            && !judge.can_concealed_kan_after_riichi(KanQuery::new(
                &self.rules,
                self.progress,
                actor,
                player,
                tile_ids,
            ))
        {
            return Err(HandError::InvalidReaction {
                reason: "the hand judge rejected this concealed kan after riichi",
            });
        }

        let mut tiles = remove_tiles(
            &mut self.players[usize::from(actor.index())].concealed,
            &tile_ids,
        );
        tiles.sort_unstable_by_key(|tile| tile.id());
        let meld = Meld::new(
            next_meld_id(&self.players[usize::from(actor.index())]),
            MeldKind::ConcealedKan,
            tiles,
            None,
            None,
        );
        let player = &mut self.players[usize::from(actor.index())];
        if player
            .drawn_tile
            .is_some_and(|drawn| tile_ids.contains(&drawn))
        {
            player.drawn_tile = None;
        }
        player.melds.push(meld.clone());

        let mut responses = self.empty_responses(actor);
        responses[usize::from(actor.index())] = Some(Reaction::Pass);
        self.phase = Phase::KanResponses(PendingKan::Concealed {
            declarer: actor,
            meld: meld.clone(),
            responses,
        });
        Ok(HandTransition::new(vec![
            HandEvent::KanProposed {
                seat: actor,
                kind: MeldKind::ConcealedKan,
                tile_kind: meld.tile_kind(),
            },
            HandEvent::MeldDeclared { seat: actor, meld },
        ]))
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

        let added_tile = remove_tiles(
            &mut self.players[usize::from(actor.index())].concealed,
            &[tile_id],
        )
        .pop()
        .expect("validated added-kan tile exists");
        let player = &mut self.players[usize::from(actor.index())];
        if player.drawn_tile == Some(tile_id) {
            player.drawn_tile = None;
        }
        let completed_meld = {
            let meld = player
                .melds
                .iter_mut()
                .find(|meld| meld.id() == meld_id)
                .expect("validated pon still exists");
            let mut tiles = std::mem::take(&mut meld.tiles).into_vec();
            tiles.push(added_tile);
            tiles.sort_unstable_by_key(|tile| tile.id());
            meld.tiles = tiles.into_boxed_slice();
            meld.kind = MeldKind::AddedKan;
            meld.clone()
        };

        let mut responses = self.empty_responses(actor);
        responses[usize::from(actor.index())] = Some(Reaction::Pass);
        self.phase = Phase::KanResponses(PendingKan::Added {
            declarer: actor,
            meld: completed_meld.clone(),
            added_tile,
            responses,
        });
        Ok(HandTransition::new(vec![
            HandEvent::KanProposed {
                seat: actor,
                kind: MeldKind::AddedKan,
                tile_kind: tile.kind(),
            },
            HandEvent::MeldDeclared {
                seat: actor,
                meld: completed_meld,
            },
        ]))
    }

    fn validate_discard_reaction(
        &self,
        pending: &PendingDiscard,
        actor: Seat,
        reaction: &Reaction,
        judge: &dyn HandJudge,
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
        if matches!(reaction, Reaction::Ron) {
            return self
                .can_ron(actor, judge)
                .map(|_| ())
                .ok_or(HandError::WinNotAllowed);
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
            Reaction::Pass | Reaction::Ron => Ok(()),
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

    fn discard_reaction_options(
        &self,
        pending: &PendingDiscard,
        actor: Seat,
        judge: &dyn HandJudge,
    ) -> Vec<Reaction> {
        let mut options = Vec::new();
        if self.can_ron(actor, judge).is_some() {
            options.push(Reaction::Ron);
        }
        if self.wall.remaining_live_draws() == 0 {
            return options;
        }
        let player = &self.players[usize::from(actor.index())];
        if !matches!(player.riichi, RiichiStatus::None) {
            return options;
        }
        let called_tile = self.players[usize::from(pending.discarder.index())].discards
            [pending.discard_index]
            .tile();
        let matching = player
            .concealed
            .iter()
            .filter(|tile| tile.kind() == called_tile.kind())
            .map(|tile| tile.id())
            .collect::<Vec<_>>();
        for left in 0..matching.len() {
            for right in left + 1..matching.len() {
                options.push(Reaction::Pon {
                    hand_tiles: [matching[left], matching[right]],
                });
            }
        }
        if matching.len() >= 3 && self.validate_kan_capacity().is_ok() {
            for first in 0..matching.len() {
                for second in first + 1..matching.len() {
                    for third in second + 1..matching.len() {
                        options.push(Reaction::OpenKan {
                            hand_tiles: [matching[first], matching[second], matching[third]],
                        });
                    }
                }
            }
        }
        let next = super::state::seat_at_offset(self.rules.variant, pending.discarder, 1);
        if matches!(self.rules.variant, RiichiVariant::Yonma) && actor == next {
            for left in 0..player.concealed.len() {
                for right in left + 1..player.concealed.len() {
                    let hand_tiles = [player.concealed[left].id(), player.concealed[right].id()];
                    if validate_chi(&player.concealed, &hand_tiles, called_tile).is_ok() {
                        options.push(Reaction::Chi { hand_tiles });
                    }
                }
            }
        }
        options
    }

    fn validate_kan_reaction(
        &self,
        pending: &PendingKan,
        actor: Seat,
        reaction: &Reaction,
        judge: &dyn HandJudge,
    ) -> Result<(), HandError> {
        if actor == pending.declarer() {
            return Err(HandError::DeclarerCannotReact);
        }
        if pending.responses()[usize::from(actor.index())].is_some() {
            return Err(HandError::AlreadyResponded { seat: actor });
        }
        match reaction {
            Reaction::Pass => Ok(()),
            Reaction::Ron => self
                .can_ron(actor, judge)
                .map(|_| ())
                .ok_or(HandError::WinNotAllowed),
            _ => Err(HandError::InvalidReaction {
                reason: "only ron or pass can respond to a kan or north extraction",
            }),
        }
    }

    fn resolve_discard_responses(
        &mut self,
        pending: &PendingDiscard,
        judge: &dyn HandJudge,
    ) -> HandTransition {
        let ron_winners = self.selected_ron_winners(
            pending.discarder,
            pending
                .responses
                .iter()
                .enumerate()
                .filter(|(_, response)| matches!(response, Some(Reaction::Ron)))
                .map(|(index, _)| {
                    Seat::new(self.rules.variant, u8::try_from(index).expect("seat index"))
                        .expect("response array only contains valid seats")
                }),
        );
        if !ron_winners.is_empty() {
            let tile = self.players[usize::from(pending.discarder.index())].discards
                [pending.discard_index]
                .tile();
            if matches!(
                self.players[usize::from(pending.discarder.index())].riichi,
                RiichiStatus::Pending
            ) {
                let player = &mut self.players[usize::from(pending.discarder.index())];
                player.riichi = RiichiStatus::None;
                player.double_riichi = false;
            }
            return self.finish_ron(
                ron_winners,
                pending.discarder,
                tile,
                WinSource::Discard {
                    from: pending.discarder,
                },
            );
        }

        let mut prefix = HandTransition::default();
        if let Some(event) = self.establish_pending_riichi(pending.discarder) {
            prefix.append(HandTransition::new(vec![event]));
        }
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
            if let Some(abortive_draw) = self.abortive_draw_after_unclaimed_discard(pending) {
                prefix.append(abortive_draw);
                return prefix;
            }
            prefix.append(self.resolve_unclaimed_discard(pending, judge));
            return prefix;
        };
        prefix.append(self.apply_discard_call(pending, caller, reaction));
        prefix
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
            Reaction::Pass | Reaction::Ron => {
                unreachable!("pass and ron cannot be selected as a call")
            }
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
        let cancelled_ippatsu = cancel_ippatsu(self.rules.variant, &mut self.players);

        let mut events = vec![HandEvent::MeldDeclared {
            seat: caller,
            meld: meld.clone(),
        }];
        if !cancelled_ippatsu.is_empty() {
            events.push(HandEvent::IppatsuCancelled {
                seats: cancelled_ippatsu,
            });
        }
        let mut transition = HandTransition::new(events);
        if matches!(kind, MeldKind::OpenKan) {
            transition.append(self.finish_kan(caller, meld));
        } else {
            let forbidden_discards =
                forbidden_after_call(self.rules.calls.kuikae, kind, called_tile, &meld);
            self.players[usize::from(caller.index())].drawn_tile = None;
            self.phase = Phase::DiscardAfterCall {
                seat: caller,
                forbidden_discards,
            };
        }
        transition
    }

    fn can_ron(&self, actor: Seat, judge: &dyn HandJudge) -> Option<(Tile, WinSource)> {
        let player = &self.players[usize::from(actor.index())];
        if player.temporary_furiten || player.riichi_furiten {
            return None;
        }
        let (tile, source) = match &self.phase {
            Phase::Responses(pending) => {
                let tile = self.players[usize::from(pending.discarder.index())].discards
                    [pending.discard_index]
                    .tile();
                (
                    tile,
                    WinSource::Discard {
                        from: pending.discarder,
                    },
                )
            }
            Phase::KanResponses(pending) => self.pending_kan_win_data(pending),
            _ => return None,
        };
        let query = WinQuery::new(
            &self.rules,
            self.progress,
            actor,
            player,
            tile,
            source,
            &self.wall,
            self.calls_occurred,
        );
        judge.can_win(query).then_some((tile, source))
    }

    fn pending_kan_win_data(&self, pending: &PendingKan) -> (Tile, WinSource) {
        match pending {
            PendingKan::Concealed { declarer, meld, .. } => {
                (meld.tiles()[0], WinSource::ConcealedKan { from: *declarer })
            }
            PendingKan::Added {
                declarer,
                meld,
                added_tile,
                ..
            } => (
                *added_tile,
                WinSource::AddedKan {
                    from: *declarer,
                    meld_id: meld.id(),
                },
            ),
            PendingKan::Nuki { declarer, tile, .. } => (*tile, WinSource::Nuki { from: *declarer }),
        }
    }

    fn apply_declined_ron(&mut self, actor: Seat) -> HandEvent {
        let player = &mut self.players[usize::from(actor.index())];
        if matches!(player.riichi, RiichiStatus::Established) {
            player.riichi_furiten = true;
        } else {
            player.temporary_furiten = true;
        }
        HandEvent::FuritenChanged {
            seat: actor,
            temporary: player.temporary_furiten,
            riichi: player.riichi_furiten,
        }
    }

    fn resolve_kan_responses(&mut self, pending: PendingKan) -> HandTransition {
        let declarer = pending.declarer();
        let winners = self.selected_ron_winners(
            declarer,
            pending
                .responses()
                .iter()
                .enumerate()
                .filter(|(_, response)| matches!(response, Some(Reaction::Ron)))
                .map(|(index, _)| {
                    Seat::new(self.rules.variant, u8::try_from(index).expect("seat index"))
                        .expect("response array only contains valid seats")
                }),
        );
        if !winners.is_empty() {
            let (tile, source) = self.pending_kan_win_data(&pending);
            return self.finish_ron(winners, declarer, tile, source);
        }
        self.complete_pending_kan(pending)
    }

    fn selected_ron_winners(
        &self,
        from: Seat,
        winners: impl IntoIterator<Item = Seat>,
    ) -> Vec<Seat> {
        let seat_count = self.rules.variant.seat_count().value();
        let mut winners: Vec<_> = winners.into_iter().collect();
        winners
            .sort_unstable_by_key(|seat| (seat.index() + seat_count - from.index()) % seat_count);
        if matches!(
            self.rules.settlement.ron_resolution,
            RonResolution::HeadBump
        ) {
            winners.truncate(1);
        }
        winners
    }

    fn finish_ron(
        &mut self,
        winners: Vec<Seat>,
        from: Seat,
        tile: Tile,
        source: WinSource,
    ) -> HandTransition {
        debug_assert!(!winners.is_empty());
        self.phase = Phase::Ended(crate::EndReason::Ron);
        HandTransition::new(vec![HandEvent::RonDeclared {
            winners: winners.into_boxed_slice(),
            from,
            tile,
            source,
        }])
    }

    pub(super) fn establish_pending_riichi(&mut self, seat: Seat) -> Option<HandEvent> {
        let player = &mut self.players[usize::from(seat.index())];
        if !matches!(player.riichi, RiichiStatus::Pending) {
            return None;
        }
        player.points -= 1_000;
        player.riichi = RiichiStatus::Established;
        player.ippatsu_eligible = self.rules.bonuses.ippatsu;
        let riichi_sticks = self
            .progress
            .deposit_riichi_stick()
            .expect("riichi declaration validated counter capacity");
        Some(HandEvent::RiichiEstablished {
            seat,
            points_after: player.points,
            riichi_sticks: riichi_sticks.value(),
        })
    }

    fn validate_turn_kan(&self, actor: Seat) -> Result<(), HandError> {
        self.validate_seat(actor)?;
        let Phase::TurnAction { seat, .. } = self.phase else {
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
        if self.wall.rinshan_draw_count() >= 4 {
            return Err(HandError::KanLimitReached);
        }
        Ok(())
    }

    fn empty_responses(&self, _declarer: Seat) -> Box<[Option<Reaction>]> {
        vec![None; usize::from(self.rules.variant.seat_count().value())].into_boxed_slice()
    }

    fn complete_pending_kan(&mut self, pending: PendingKan) -> HandTransition {
        match pending {
            PendingKan::Concealed { declarer, meld, .. }
            | PendingKan::Added { declarer, meld, .. } => self.finish_kan(declarer, meld),
            PendingKan::Nuki { declarer, .. } => self.complete_nuki(declarer),
        }
    }

    fn finish_kan(&mut self, actor: Seat, meld: Meld) -> HandTransition {
        self.kan_counts[usize::from(actor.index())] += 1;
        self.calls_occurred = true;
        let cancelled_ippatsu = cancel_ippatsu(self.rules.variant, &mut self.players);

        let mut events = vec![HandEvent::KanCompleted { seat: actor, meld }];
        if !cancelled_ippatsu.is_empty() {
            events.push(HandEvent::IppatsuCancelled {
                seats: cancelled_ippatsu,
            });
        }
        if self.should_abort_for_four_kans() {
            let mut transition = HandTransition::new(events);
            transition.append(self.finish_abortive_draw(EndReason::FourKans, None));
            return transition;
        }
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
        let cleared_temporary_furiten = player.temporary_furiten;
        player.concealed.push(tile);
        player.drawn_tile = Some(tile.id());
        player.temporary_furiten = false;
        self.phase = Phase::TurnAction {
            seat: actor,
            draw_source: DrawSource::Rinshan,
        };
        if cleared_temporary_furiten {
            events.push(HandEvent::FuritenChanged {
                seat: actor,
                temporary: false,
                riichi: player.riichi_furiten,
            });
        }
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
        Reaction::Ron => ReactionKind::Ron,
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

fn forbidden_after_call(
    kuikae: KuikaeRule,
    kind: MeldKind,
    called_tile: Tile,
    meld: &Meld,
) -> Box<[TileKind]> {
    if matches!(kuikae, KuikaeRule::Allowed) {
        return Box::default();
    }
    let mut forbidden = vec![called_tile.kind()];
    if matches!(kuikae, KuikaeRule::Forbidden) && matches!(kind, MeldKind::Chi) {
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

fn cancel_ippatsu(variant: RiichiVariant, players: &mut [crate::PlayerHand]) -> Box<[Seat]> {
    let mut cancelled = Vec::new();
    for (index, player) in players.iter_mut().enumerate() {
        if player.ippatsu_eligible {
            cancelled.push(
                Seat::new(variant, u8::try_from(index).expect("seat index"))
                    .expect("player array only contains valid seats"),
            );
            player.ippatsu_eligible = false;
        }
    }
    cancelled.into_boxed_slice()
}

fn reaction_priority(reaction: &Reaction) -> u8 {
    match reaction {
        Reaction::Ron => 3,
        Reaction::Pon { .. } | Reaction::OpenKan { .. } => 2,
        Reaction::Chi { .. } => 1,
        Reaction::Pass => 0,
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        DrawSource, EndReason, HandEvent, HandJudge, HandPhase, KuikaeRule, MeldKind, Reaction,
        RejectAllHandJudge, RiichiHand, RiichiRules, RiichiStatus, RiichiVariant, RonResolution,
        Seat, TableProgress, TileId, TileKind, WallSeed, WinQuery, WinSource,
    };

    use super::{Phase, forbidden_after_call, validate_chi};

    struct AcceptWins;

    impl HandJudge for AcceptWins {
        fn can_win(&self, _query: WinQuery<'_>) -> bool {
            true
        }
    }

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

    fn sanma_with_dealer_north() -> (RiichiHand, Seat, TileId) {
        let variant = RiichiVariant::Sanma;
        let dealer = Seat::new(variant, 0).expect("dealer");
        let progress = TableProgress::east_one(variant, dealer).expect("progress");
        for seed in 0_u8..=u8::MAX {
            let hand = RiichiHand::start(
                RiichiRules::standard(variant),
                progress,
                [25_000; 3],
                &WallSeed::from_bytes([seed; 32]),
            )
            .expect("start")
            .0;
            if let Some(tile_id) = hand.players[0]
                .concealed
                .iter()
                .find(|tile| tile.kind() == TileKind::honor(crate::Honor::North))
                .map(|tile| tile.id())
            {
                return (hand, dealer, tile_id);
            }
        }
        panic!("one deterministic seed should deal north to the dealer");
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

    fn find_uncallable_discard() -> (RiichiHand, Seat, TileId) {
        let dealer = Seat::new(RiichiVariant::Yonma, 0).expect("dealer");
        for seed in 0..10_000 {
            let hand = start_yonma(seed);
            let discards = hand.player(dealer).expect("dealer").concealed().to_vec();
            for discard in discards {
                let mut candidate = hand.clone();
                candidate
                    .discard(dealer, discard.id())
                    .expect("candidate discard");
                let no_reactions = (1..4).all(|index| {
                    let seat = Seat::new(RiichiVariant::Yonma, index).expect("opponent");
                    candidate
                        .available_reactions(seat, &RejectAllHandJudge)
                        .expect("reaction options")
                        .is_empty()
                });
                if no_reactions {
                    return (hand, dealer, discard.id());
                }
            }
        }
        panic!("deterministic seed search did not find an uncallable discard");
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
                        let forbidden = forbidden_after_call(
                            KuikaeRule::Forbidden,
                            MeldKind::Chi,
                            discard,
                            &meld,
                        );
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
                last = Some(hand.pass(seat, &RejectAllHandJudge).expect("pass"));
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
            &RejectAllHandJudge,
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
    fn unavailable_reactions_are_skipped_and_next_player_draws_immediately() {
        let (mut hand, dealer, discard_id) = find_uncallable_discard();
        let next = Seat::new(RiichiVariant::Yonma, 1).expect("next seat");
        hand.discard(dealer, discard_id).expect("discard");

        let transition = hand
            .advance_automatic_reactions(&RejectAllHandJudge)
            .expect("automatic reactions");

        assert_eq!(hand.phase(), HandPhase::AwaitingTurnAction { seat: next });
        assert!(
            transition
                .events()
                .iter()
                .any(|event| matches!(event, HandEvent::TileDrawn { seat, .. } if *seat == next))
        );
    }

    #[test]
    fn reaction_options_include_exact_pon_tile_ids() {
        let (mut hand, caller, discard_id, matching) = find_matching_scenario(2);
        let dealer = Seat::new(RiichiVariant::Yonma, 0).expect("dealer");
        hand.discard(dealer, discard_id).expect("discard");

        let options = hand
            .available_reactions(caller, &RejectAllHandJudge)
            .expect("reaction options");

        assert!(options.iter().any(|reaction| {
            matches!(
                reaction,
                Reaction::Pon { hand_tiles }
                    if hand_tiles.contains(&matching[0]) && hand_tiles.contains(&matching[1])
            )
        }));
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
            &RejectAllHandJudge,
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
            &RejectAllHandJudge,
        )
        .expect("chi response arrives first");
        hand.respond(
            pon_seat,
            Reaction::Pon {
                hand_tiles: pon_tiles,
            },
            &RejectAllHandJudge,
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
            &RejectAllHandJudge,
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
    fn concealed_and_added_kan_are_placed_before_responses_and_draw_after_passes() {
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
        concealed_hand.players[usize::from(dealer.index())].drawn_tile = None;
        concealed_hand.players[usize::from(actor.index())].drawn_tile = Some(fourth.id());
        concealed_hand.phase = Phase::TurnAction {
            seat: actor,
            draw_source: DrawSource::LiveWall,
        };

        concealed_hand
            .declare_concealed_kan(
                actor,
                [matching[0], matching[1], matching[2], fourth.id()],
                &RejectAllHandJudge,
            )
            .expect("propose concealed kan");
        assert_eq!(
            concealed_hand.player(actor).expect("actor").melds()[0].kind(),
            MeldKind::ConcealedKan
        );
        assert_eq!(concealed_hand.wall.rinshan_draw_count(), 0);
        concealed_hand
            .validate_invariants()
            .expect("placed concealed kan remains valid during responses");
        pass_remaining(&mut concealed_hand, actor, &[]);
        assert_eq!(
            concealed_hand.player(actor).expect("actor").melds()[0].kind(),
            MeldKind::ConcealedKan
        );
        assert_eq!(concealed_hand.wall.rinshan_draw_count(), 1);

        let (mut added_hand, caller, discard_id, matching) = find_matching_scenario(3);
        added_hand.discard(dealer, discard_id).expect("discard");
        added_hand
            .respond(
                caller,
                Reaction::Pon {
                    hand_tiles: [matching[0], matching[1]],
                },
                &RejectAllHandJudge,
            )
            .expect("pon");
        pass_remaining(&mut added_hand, dealer, &[caller]);
        added_hand.phase = Phase::TurnAction {
            seat: caller,
            draw_source: DrawSource::LiveWall,
        };
        added_hand.players[usize::from(caller.index())].drawn_tile = Some(matching[2]);

        added_hand
            .declare_added_kan(caller, crate::MeldId::new(0), matching[2])
            .expect("propose added kan");
        assert_eq!(
            added_hand.player(caller).expect("caller").melds()[0].kind(),
            MeldKind::AddedKan
        );
        assert_eq!(added_hand.wall.rinshan_draw_count(), 0);
        added_hand
            .validate_invariants()
            .expect("placed added kan remains valid during responses");
        pass_remaining(&mut added_hand, caller, &[]);
        assert_eq!(
            added_hand.player(caller).expect("caller").melds()[0].kind(),
            MeldKind::AddedKan
        );
        assert_eq!(added_hand.wall.rinshan_draw_count(), 1);
    }

    #[test]
    fn ron_on_added_kan_keeps_placed_tile_and_prevents_rinshan_draw() {
        let (mut hand, declarer, discard_id, matching) = find_matching_scenario(3);
        let dealer = Seat::new(RiichiVariant::Yonma, 0).expect("dealer");
        hand.discard(dealer, discard_id).expect("discard");
        hand.respond(
            declarer,
            Reaction::Pon {
                hand_tiles: [matching[0], matching[1]],
            },
            &RejectAllHandJudge,
        )
        .expect("pon");
        pass_remaining(&mut hand, dealer, &[declarer]);
        hand.phase = Phase::TurnAction {
            seat: declarer,
            draw_source: DrawSource::LiveWall,
        };
        hand.players[usize::from(declarer.index())].drawn_tile = Some(matching[2]);
        hand.declare_added_kan(declarer, crate::MeldId::new(0), matching[2])
            .expect("added kan");

        hand.respond(dealer, Reaction::Ron, &AcceptWins)
            .expect("chankan");
        let transition = pass_remaining(&mut hand, declarer, &[dealer]);

        assert_eq!(
            hand.phase(),
            HandPhase::Ended {
                reason: EndReason::Ron
            }
        );
        assert_eq!(
            hand.player(declarer).expect("declarer").melds()[0].kind(),
            MeldKind::AddedKan
        );
        assert_eq!(hand.kan_counts[usize::from(declarer.index())], 0);
        assert_eq!(hand.wall.rinshan_draw_count(), 0);
        assert!(matches!(
            transition.events().last(),
            Some(HandEvent::RonDeclared {
                source: WinSource::AddedKan { from, .. },
                ..
            }) if *from == declarer
        ));
        hand.validate_invariants()
            .expect("robbed added kan remains a valid placed state");
    }

    #[test]
    fn ron_on_nuki_keeps_extracted_north_and_prevents_replacement_draw() {
        let (mut hand, declarer, north_id) = sanma_with_dealer_north();
        let responder = Seat::new(RiichiVariant::Sanma, 1).expect("responder");
        let other = Seat::new(RiichiVariant::Sanma, 2).expect("other responder");
        let remaining = hand.remaining_live_draws();

        hand.declare_nuki(declarer, north_id)
            .expect("propose north extraction");
        assert_eq!(
            hand.available_reactions(responder, &AcceptWins)
                .expect("reaction options"),
            vec![Reaction::Ron]
        );
        hand.respond(responder, Reaction::Ron, &AcceptWins)
            .expect("rob north");
        let transition = hand.pass(other, &AcceptWins).expect("other player passes");

        assert_eq!(
            hand.phase(),
            HandPhase::Ended {
                reason: EndReason::Ron
            }
        );
        assert_eq!(
            hand.player(declarer).expect("declarer").nuki_tiles()[0].id(),
            north_id
        );
        assert!(
            !hand.players[usize::from(declarer.index())]
                .concealed
                .iter()
                .any(|tile| tile.id() == north_id)
        );
        assert_eq!(hand.wall.rinshan_draw_count(), 0);
        assert_eq!(hand.remaining_live_draws(), remaining);
        assert!(matches!(
            transition.events().last(),
            Some(HandEvent::RonDeclared {
                source: WinSource::Nuki { from },
                tile,
                ..
            }) if *from == declarer && tile.id() == north_id
        ));
        hand.validate_invariants()
            .expect("robbed north remains a valid extracted state");
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
                },
                &RejectAllHandJudge,
            )
            .is_err()
        );
        assert_eq!(hand, before);
    }

    #[test]
    fn ron_priority_is_rule_driven_not_arrival_order() {
        let dealer = Seat::new(RiichiVariant::Yonma, 0).expect("dealer");
        let seat_one = Seat::new(RiichiVariant::Yonma, 1).expect("seat one");
        let seat_two = Seat::new(RiichiVariant::Yonma, 2).expect("seat two");
        let seat_three = Seat::new(RiichiVariant::Yonma, 3).expect("seat three");

        let mut multiple = start_yonma(77);
        let discard = multiple
            .player(dealer)
            .expect("dealer")
            .drawn_tile_id()
            .expect("draw");
        multiple.discard(dealer, discard).expect("discard");
        multiple
            .respond(seat_two, Reaction::Ron, &AcceptWins)
            .expect("second seat ron arrives first");
        multiple
            .respond(seat_one, Reaction::Ron, &AcceptWins)
            .expect("nearest ron arrives second");
        let transition = multiple
            .pass(seat_three, &AcceptWins)
            .expect("last response");
        assert_eq!(
            multiple.phase(),
            HandPhase::Ended {
                reason: EndReason::Ron
            }
        );
        assert!(matches!(
            transition.events().last(),
            Some(HandEvent::RonDeclared { winners, .. })
                if winners.as_ref() == [seat_one, seat_two]
        ));

        let mut head_bump = start_yonma(77);
        head_bump.rules.settlement.ron_resolution = RonResolution::HeadBump;
        let discard = head_bump
            .player(dealer)
            .expect("dealer")
            .drawn_tile_id()
            .expect("draw");
        head_bump.discard(dealer, discard).expect("discard");
        head_bump
            .respond(seat_two, Reaction::Ron, &AcceptWins)
            .expect("ron");
        head_bump
            .respond(seat_one, Reaction::Ron, &AcceptWins)
            .expect("ron");
        let transition = head_bump
            .pass(seat_three, &AcceptWins)
            .expect("last response");
        assert!(matches!(
            transition.events().last(),
            Some(HandEvent::RonDeclared { winners, .. })
                if winners.as_ref() == [seat_one]
        ));
    }

    #[test]
    fn declining_ron_sets_temporary_furiten_without_affecting_other_responses() {
        let mut hand = start_yonma(91);
        let dealer = Seat::new(RiichiVariant::Yonma, 0).expect("dealer");
        let actor = Seat::new(RiichiVariant::Yonma, 2).expect("actor");
        let discard = hand
            .player(dealer)
            .expect("dealer")
            .drawn_tile_id()
            .expect("draw");
        hand.discard(dealer, discard).expect("discard");

        let transition = hand.pass(actor, &AcceptWins).expect("decline ron");

        assert!(hand.player(actor).expect("actor").is_temporary_furiten());
        assert!(transition.events().iter().any(|event| matches!(
            event,
            HandEvent::FuritenChanged {
                seat,
                temporary: true,
                ..
            } if *seat == actor
        )));
        assert_eq!(
            hand.phase(),
            HandPhase::AwaitingResponses {
                trigger_seat: dealer
            }
        );
    }

    #[test]
    fn declining_ron_after_riichi_sets_persistent_riichi_furiten() {
        let mut hand = start_yonma(92);
        let dealer = Seat::new(RiichiVariant::Yonma, 0).expect("dealer");
        let actor = Seat::new(RiichiVariant::Yonma, 2).expect("actor");
        hand.players[usize::from(actor.index())].riichi = RiichiStatus::Established;
        let discard = hand
            .player(dealer)
            .expect("dealer")
            .drawn_tile_id()
            .expect("draw");
        hand.discard(dealer, discard).expect("discard");

        hand.pass(actor, &AcceptWins).expect("decline ron");

        assert!(!hand.player(actor).expect("actor").is_temporary_furiten());
        assert!(hand.player(actor).expect("actor").is_riichi_furiten());
    }

    #[test]
    fn next_draw_clears_temporary_furiten() {
        let mut hand = start_yonma(93);
        let dealer = Seat::new(RiichiVariant::Yonma, 0).expect("dealer");
        let next = Seat::new(RiichiVariant::Yonma, 1).expect("next");
        let discard = hand
            .player(dealer)
            .expect("dealer")
            .drawn_tile_id()
            .expect("draw");
        hand.discard(dealer, discard).expect("discard");
        hand.pass(next, &AcceptWins).expect("decline ron");
        hand.pass(
            Seat::new(RiichiVariant::Yonma, 2).expect("seat"),
            &RejectAllHandJudge,
        )
        .expect("pass");
        let transition = hand
            .pass(
                Seat::new(RiichiVariant::Yonma, 3).expect("seat"),
                &RejectAllHandJudge,
            )
            .expect("last pass");

        assert!(!hand.player(next).expect("next").is_temporary_furiten());
        assert!(transition.events().iter().any(|event| matches!(
            event,
            HandEvent::FuritenChanged {
                seat,
                temporary: false,
                ..
            } if *seat == next
        )));
    }

    #[test]
    fn call_helpers_reject_impossible_empty_inputs() {
        let called = tile(99, "3p");

        assert!(
            validate_chi(&[], &[crate::TileId::new(1), crate::TileId::new(2)], called).is_err()
        );
    }

    fn tile(id: u16, code: &str) -> crate::Tile {
        crate::Tile::new(
            crate::TileId::new(id),
            code.parse::<TileKind>().expect("tile kind"),
            false,
        )
        .expect("tile")
    }

    /// Builds the meld a call would produce; `codes` are the meld tiles in order.
    fn meld_of(kind: MeldKind, codes: &[&str], called: crate::Tile) -> crate::Meld {
        crate::Meld::new(
            crate::MeldId::new(0),
            kind,
            codes
                .iter()
                .enumerate()
                .map(|(index, code)| tile(u16::try_from(index).expect("tile id"), code))
                .collect::<Vec<_>>(),
            Some(Seat::new(RiichiVariant::Yonma, 0).expect("seat")),
            Some(called.id()),
        )
    }

    fn kinds(codes: &[&str]) -> Vec<TileKind> {
        codes
            .iter()
            .map(|code| code.parse::<TileKind>().expect("tile kind"))
            .collect()
    }

    #[test]
    fn kuikae_rule_selects_which_discards_a_call_forbids() {
        let called = tile(99, "3p");
        let meld = meld_of(MeldKind::Chi, &["3p", "4p", "5p"], called);

        assert_eq!(
            forbidden_after_call(KuikaeRule::Forbidden, MeldKind::Chi, called, &meld).as_ref(),
            kinds(&["3p", "6p"])
        );
        assert_eq!(
            forbidden_after_call(KuikaeRule::SameTileOnly, MeldKind::Chi, called, &meld).as_ref(),
            kinds(&["3p"])
        );
        assert!(forbidden_after_call(KuikaeRule::Allowed, MeldKind::Chi, called, &meld).is_empty());
    }

    #[test]
    fn suji_kuikae_needs_a_chi_that_extends_a_sequence_end() {
        let upper = tile(99, "6p");
        let upper_meld = meld_of(MeldKind::Chi, &["4p", "5p", "6p"], upper);
        assert_eq!(
            forbidden_after_call(KuikaeRule::Forbidden, MeldKind::Chi, upper, &upper_meld).as_ref(),
            kinds(&["3p", "6p"])
        );

        let middle = tile(99, "3p");
        let middle_meld = meld_of(MeldKind::Chi, &["2p", "3p", "4p"], middle);
        assert_eq!(
            forbidden_after_call(KuikaeRule::Forbidden, MeldKind::Chi, middle, &middle_meld)
                .as_ref(),
            kinds(&["3p"])
        );

        let edge = tile(99, "3p");
        let edge_meld = meld_of(MeldKind::Chi, &["1p", "2p", "3p"], edge);
        assert_eq!(
            forbidden_after_call(KuikaeRule::Forbidden, MeldKind::Chi, edge, &edge_meld).as_ref(),
            kinds(&["3p"])
        );

        let ponned = tile(99, "3p");
        let pon_meld = meld_of(MeldKind::Pon, &["3p", "3p", "3p"], ponned);
        assert_eq!(
            forbidden_after_call(KuikaeRule::Forbidden, MeldKind::Pon, ponned, &pon_meld).as_ref(),
            kinds(&["3p"])
        );
    }
}
