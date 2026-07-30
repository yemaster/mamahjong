use crate::{
    EndReason, HandError, HandEvent, HandTransition, Honor, RiichiHand, RiichiStatus,
    RiichiVariant, Seat, TileKind,
};

use super::state::{PendingDiscard, Phase};

impl RiichiHand {
    pub fn declare_nine_terminals(&mut self, actor: Seat) -> Result<HandTransition, HandError> {
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
        if !self.rules.abortive_draws.nine_terminals {
            return Err(HandError::AbortiveDrawNotAllowed {
                reason: "nine terminals is disabled by the active rules",
            });
        }
        let player = &self.players[usize::from(actor.index())];
        if self.calls_occurred || !player.discards.is_empty() || !player.melds.is_empty() {
            return Err(HandError::AbortiveDrawNotAllowed {
                reason: "nine terminals is only available on the player's uninterrupted first turn",
            });
        }

        let mut distinct = [false; TileKind::COUNT];
        for tile in &player.concealed {
            if tile.kind().is_terminal_or_honor() {
                distinct[tile.kind().index()] = true;
            }
        }
        if distinct.into_iter().filter(|present| *present).count() < 9 {
            return Err(HandError::AbortiveDrawNotAllowed {
                reason: "the hand contains fewer than nine distinct terminal or honor kinds",
            });
        }

        Ok(self.finish_abortive_draw(EndReason::NineTerminals, Some(actor)))
    }

    pub(super) fn abortive_draw_after_unclaimed_discard(
        &mut self,
        pending: &PendingDiscard,
    ) -> Option<HandTransition> {
        if self.is_four_winds() {
            return Some(self.finish_abortive_draw(EndReason::FourWinds, None));
        }
        if self.is_four_riichi() {
            return Some(self.finish_abortive_draw(EndReason::FourRiichi, None));
        }
        debug_assert_eq!(
            self.players[usize::from(pending.discarder.index())]
                .discards
                .len(),
            pending.discard_index + 1
        );
        None
    }

    pub(super) fn should_abort_for_four_kans(&self) -> bool {
        self.rules.abortive_draws.four_kans
            && self.kan_counts.iter().copied().sum::<u8>() == 4
            && self.kan_counts.iter().filter(|count| **count > 0).count() >= 2
    }

    pub(super) fn finish_abortive_draw(
        &mut self,
        reason: EndReason,
        declarer: Option<Seat>,
    ) -> HandTransition {
        debug_assert!(matches!(
            reason,
            EndReason::NineTerminals
                | EndReason::FourWinds
                | EndReason::FourKans
                | EndReason::FourRiichi
        ));
        self.phase = Phase::Ended(reason);
        HandTransition::new(vec![HandEvent::AbortiveDrawDeclared { reason, declarer }])
    }

    fn is_four_winds(&self) -> bool {
        if !self.rules.abortive_draws.four_winds
            || !matches!(self.rules.variant, RiichiVariant::Yonma)
            || self.calls_occurred
            || self.players.iter().any(|player| player.discards.len() != 1)
        {
            return false;
        }

        let first_kind = self.players[0].discards[0].tile().kind();
        matches!(
            first_kind.honor_value(),
            Some(Honor::East | Honor::South | Honor::West | Honor::North)
        ) && self
            .players
            .iter()
            .all(|player| player.discards[0].tile().kind() == first_kind)
    }

    fn is_four_riichi(&self) -> bool {
        self.rules.abortive_draws.four_riichi
            && matches!(self.rules.variant, RiichiVariant::Yonma)
            && self
                .players
                .iter()
                .all(|player| matches!(player.riichi, RiichiStatus::Established))
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        Discard, EndReason, HandEvent, HandPhase, Reaction, RiichiHand, RiichiRules, RiichiStatus,
        RiichiVariant, Seat, TableProgress, Tile, TileId, TileKind, WallSeed,
    };

    use super::super::state::PendingDiscard;

    fn start() -> RiichiHand {
        let variant = RiichiVariant::Yonma;
        let dealer = Seat::new(variant, 0).expect("dealer");
        let progress = TableProgress::east_one(variant, dealer).expect("progress");
        RiichiHand::start(
            RiichiRules::standard(variant),
            progress,
            [25_000; 4],
            &WallSeed::from_bytes([31; 32]),
        )
        .expect("start")
        .0
    }

    fn pending_after_first_discard(hand: &mut RiichiHand, kind: TileKind) -> PendingDiscard {
        for index in 0..4 {
            let seat = Seat::new(RiichiVariant::Yonma, index).expect("seat");
            let tile = Tile::new(TileId::new(200 + u16::from(index)), kind, false).expect("tile");
            hand.players[usize::from(index)]
                .discards
                .push(Discard::new(tile, false, false));
            hand.players[usize::from(index)].drawn_tile = None;
            hand.players[usize::from(index)].riichi = RiichiStatus::None;
            debug_assert_eq!(seat.index(), index);
        }
        PendingDiscard {
            discarder: Seat::new(RiichiVariant::Yonma, 3).expect("discarder"),
            discard_index: 0,
            responses: vec![Some(Reaction::Pass); 4].into_boxed_slice(),
        }
    }

    #[test]
    fn nine_terminals_requires_first_turn_and_nine_distinct_kinds() {
        let mut hand = start();
        let dealer = Seat::new(RiichiVariant::Yonma, 0).expect("dealer");
        let kinds = ["1m", "9m", "1p", "9p", "1s", "9s", "1z", "2z", "3z"];
        for (tile, code) in hand.players[0].concealed.iter_mut().zip(kinds) {
            *tile = Tile::new(tile.id(), code.parse().expect("tile kind"), false).expect("tile");
        }

        let transition = hand.declare_nine_terminals(dealer).expect("nine terminals");

        assert_eq!(
            hand.phase(),
            HandPhase::Ended {
                reason: EndReason::NineTerminals
            }
        );
        assert!(matches!(
            transition.events(),
            [HandEvent::AbortiveDrawDeclared {
                reason: EndReason::NineTerminals,
                declarer: Some(seat),
            }] if *seat == dealer
        ));
    }

    #[test]
    fn rejected_nine_terminals_is_atomic() {
        let mut hand = start();
        let dealer = Seat::new(RiichiVariant::Yonma, 0).expect("dealer");
        let before = hand.clone();

        assert!(hand.declare_nine_terminals(dealer).is_err());
        assert_eq!(hand, before);
    }

    #[test]
    fn identical_first_wind_discards_abort_yonma() {
        let mut hand = start();
        let pending = pending_after_first_discard(&mut hand, "1z".parse().expect("east tile kind"));

        let transition = hand
            .abortive_draw_after_unclaimed_discard(&pending)
            .expect("four winds");

        assert_eq!(
            hand.phase(),
            HandPhase::Ended {
                reason: EndReason::FourWinds
            }
        );
        assert!(matches!(
            transition.events(),
            [HandEvent::AbortiveDrawDeclared {
                reason: EndReason::FourWinds,
                declarer: None,
            }]
        ));
    }

    #[test]
    fn four_established_riichi_abort_after_ron_window() {
        let mut hand = start();
        let pending = pending_after_first_discard(&mut hand, "5p".parse().expect("tile kind"));
        for player in &mut hand.players {
            player.riichi = RiichiStatus::Established;
        }

        let transition = hand
            .abortive_draw_after_unclaimed_discard(&pending)
            .expect("four riichi");

        assert!(matches!(
            transition.events(),
            [HandEvent::AbortiveDrawDeclared {
                reason: EndReason::FourRiichi,
                declarer: None,
            }]
        ));
    }

    #[test]
    fn four_kans_requires_at_least_two_declarers() {
        let mut hand = start();
        hand.kan_counts.copy_from_slice(&[4, 0, 0, 0]);
        assert!(!hand.should_abort_for_four_kans());

        hand.kan_counts.copy_from_slice(&[3, 1, 0, 0]);
        assert!(hand.should_abort_for_four_kans());
    }
}
