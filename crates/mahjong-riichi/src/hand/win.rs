use crate::{
    EndReason, HandError, HandEvent, HandJudge, HandTransition, MeldKind, RiichiHand, RiichiQuery,
    RiichiStatus, Seat, TileId, WinQuery, WinSource,
};

use super::state::Phase;

impl RiichiHand {
    pub fn declare_riichi_and_discard(
        &mut self,
        actor: Seat,
        tile_id: TileId,
        judge: &dyn HandJudge,
    ) -> Result<HandTransition, HandError> {
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

        let player = &self.players[usize::from(actor.index())];
        if !matches!(player.riichi, RiichiStatus::None) {
            return Err(HandError::RiichiNotAllowed {
                reason: "riichi was already declared",
            });
        }
        if player
            .melds
            .iter()
            .any(|meld| !matches!(meld.kind(), MeldKind::ConcealedKan))
        {
            return Err(HandError::RiichiNotAllowed {
                reason: "riichi requires a closed hand",
            });
        }
        if player.points < 1_000 {
            return Err(HandError::RiichiNotAllowed {
                reason: "riichi requires at least 1000 points",
            });
        }
        if self.wall.remaining_live_draws() == 0 {
            return Err(HandError::RiichiNotAllowed {
                reason: "riichi is unavailable after the last live-wall draw",
            });
        }
        self.progress
            .riichi_sticks()
            .checked_deposit()
            .map_err(|_| HandError::RiichiNotAllowed {
                reason: "riichi-stick counter cannot be incremented",
            })?;
        let discard_tile = player
            .concealed
            .iter()
            .find(|tile| tile.id() == tile_id)
            .copied()
            .ok_or(HandError::TileNotInHand { tile_id })?;
        if !judge.can_riichi(RiichiQuery::new(
            &self.rules,
            self.progress,
            actor,
            player,
            discard_tile,
        )) {
            return Err(HandError::RiichiNotAllowed {
                reason: "the hand judge rejected the riichi declaration",
            });
        }

        self.discard_internal(actor, tile_id, true)
    }

    pub fn declare_tsumo(
        &mut self,
        actor: Seat,
        judge: &dyn HandJudge,
    ) -> Result<HandTransition, HandError> {
        self.validate_seat(actor)?;
        let Phase::TurnAction { seat, draw_source } = self.phase else {
            return Err(HandError::WrongPhase);
        };
        if actor != seat {
            return Err(HandError::NotActiveSeat {
                expected: seat,
                actual: actor,
            });
        }
        let player = &self.players[usize::from(actor.index())];
        let drawn_tile_id = player
            .drawn_tile
            .expect("turn-action phase always contains a drawn tile");
        let tile = player
            .concealed
            .iter()
            .find(|tile| tile.id() == drawn_tile_id)
            .copied()
            .expect("drawn tile remains in concealed hand");
        if !judge.can_win(WinQuery::new(
            &self.rules,
            self.progress,
            actor,
            player,
            tile,
            WinSource::Tsumo(draw_source),
        )) {
            return Err(HandError::WinNotAllowed);
        }

        self.phase = Phase::Ended(EndReason::Tsumo);
        Ok(HandTransition::new(vec![HandEvent::TsumoDeclared {
            winner: actor,
            tile,
            source: draw_source,
        }]))
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        EndReason, HandEvent, HandJudge, HandPhase, RejectAllHandJudge, RiichiHand, RiichiRules,
        RiichiVariant, Seat, TableProgress, WallSeed, WinQuery,
    };

    struct WinningJudge;

    impl HandJudge for WinningJudge {
        fn can_win(&self, _query: WinQuery<'_>) -> bool {
            true
        }

        fn can_riichi(&self, _query: crate::RiichiQuery<'_>) -> bool {
            true
        }
    }

    fn start() -> RiichiHand {
        let dealer = Seat::new(RiichiVariant::Yonma, 0).expect("dealer");
        let progress = TableProgress::east_one(RiichiVariant::Yonma, dealer).expect("progress");
        RiichiHand::start(
            RiichiRules::default(),
            progress,
            [25_000; 4],
            &WallSeed::from_bytes([21; 32]),
        )
        .expect("start")
        .0
    }

    #[test]
    fn tsumo_ends_hand_only_when_judge_accepts() {
        let mut rejected = start();
        let dealer = Seat::new(RiichiVariant::Yonma, 0).expect("dealer");
        let before = rejected.clone();
        assert!(rejected.declare_tsumo(dealer, &RejectAllHandJudge).is_err());
        assert_eq!(rejected, before);

        let transition = rejected
            .declare_tsumo(dealer, &WinningJudge)
            .expect("tsumo");
        assert_eq!(
            rejected.phase(),
            HandPhase::Ended {
                reason: EndReason::Tsumo
            }
        );
        assert!(matches!(
            transition.events(),
            [HandEvent::TsumoDeclared { winner, .. }] if *winner == dealer
        ));
    }

    #[test]
    fn riichi_establishes_only_after_declaration_tile_survives() {
        let mut hand = start();
        let dealer = Seat::new(RiichiVariant::Yonma, 0).expect("dealer");
        let tile_id = hand.player(dealer).expect("dealer").concealed()[0].id();

        let declaration = hand
            .declare_riichi_and_discard(dealer, tile_id, &WinningJudge)
            .expect("declare riichi");

        assert!(matches!(
            declaration.events(),
            [HandEvent::TileDiscarded {
                riichi_declared: true,
                ..
            }]
        ));
        assert_eq!(
            hand.player(dealer).expect("dealer").riichi_status(),
            crate::RiichiStatus::Pending
        );
        assert_eq!(hand.player(dealer).expect("dealer").points(), 25_000);

        let mut final_transition = None;
        for index in 1..4 {
            let seat = Seat::new(RiichiVariant::Yonma, index).expect("seat");
            final_transition = Some(hand.pass(seat, &WinningJudge).expect("pass"));
        }

        assert_eq!(
            hand.player(dealer).expect("dealer").riichi_status(),
            crate::RiichiStatus::Established
        );
        assert_eq!(hand.player(dealer).expect("dealer").points(), 24_000);
        assert_eq!(hand.progress().riichi_sticks().value(), 1);
        assert!(hand.player(dealer).expect("dealer").is_ippatsu_eligible());
        assert!(final_transition
            .expect("final response")
            .events()
            .iter()
            .any(|event| matches!(event, HandEvent::RiichiEstablished { seat, .. } if *seat == dealer)));
    }

    #[test]
    fn rejected_riichi_is_atomic() {
        let mut hand = start();
        let dealer = Seat::new(RiichiVariant::Yonma, 0).expect("dealer");
        let tile_id = hand.player(dealer).expect("dealer").concealed()[0].id();
        let before = hand.clone();

        assert!(
            hand.declare_riichi_and_discard(dealer, tile_id, &RejectAllHandJudge)
                .is_err()
        );
        assert_eq!(hand, before);
    }

    #[test]
    fn ron_on_declaration_tile_cancels_pending_riichi() {
        let mut hand = start();
        let dealer = Seat::new(RiichiVariant::Yonma, 0).expect("dealer");
        let winner = Seat::new(RiichiVariant::Yonma, 1).expect("winner");
        let tile_id = hand.player(dealer).expect("dealer").concealed()[0].id();
        hand.declare_riichi_and_discard(dealer, tile_id, &WinningJudge)
            .expect("declare");
        hand.respond(winner, crate::Reaction::Ron, &WinningJudge)
            .expect("ron");
        for index in 2..4 {
            hand.pass(
                Seat::new(RiichiVariant::Yonma, index).expect("seat"),
                &WinningJudge,
            )
            .expect("pass");
        }

        assert_eq!(
            hand.player(dealer).expect("dealer").riichi_status(),
            crate::RiichiStatus::None
        );
        assert_eq!(hand.player(dealer).expect("dealer").points(), 25_000);
        assert_eq!(hand.progress().riichi_sticks().value(), 0);
        assert_eq!(
            hand.phase(),
            HandPhase::Ended {
                reason: EndReason::Ron
            }
        );
    }

    #[test]
    fn established_riichi_rejects_tedashi_atomically() {
        let mut hand = start();
        let dealer = Seat::new(RiichiVariant::Yonma, 0).expect("dealer");
        hand.players[usize::from(dealer.index())].riichi = crate::RiichiStatus::Established;
        let drawn = hand
            .player(dealer)
            .expect("dealer")
            .drawn_tile_id()
            .expect("drawn");
        let tedashi = hand
            .player(dealer)
            .expect("dealer")
            .concealed()
            .iter()
            .find(|tile| tile.id() != drawn)
            .expect("another tile")
            .id();
        let before = hand.clone();

        assert!(hand.discard(dealer, tedashi).is_err());
        assert_eq!(hand, before);
    }
}
