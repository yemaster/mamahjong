import { useEffect, useRef, useState } from "react";
import { playSfx, NEWROUND_PAIS_SFX } from "../audio/sfx";
import { useSceneWaitingPeers } from "../components/SceneTransition";
import type { GameCommandName, MatchView } from "../types";
import {
  OPENING_DICE_MS,
  TILE_STAND_UP_MS,
  openingDealArrival,
  openingDealDuration,
} from "./table";
import type { OpeningPhase } from "./OpeningSequence";

interface MatchOpeningOptions {
  matchId: string;
  view: MatchView | null;
  assetsLoaded: boolean;
  onCommand: (name: GameCommandName, payload?: unknown) => void;
}

interface MatchOpeningState {
  phase: OpeningPhase;
  allAssetsReady: boolean;
  assetsTimedOut: boolean;
}

/**
 * Runs the rule-independent client opening flow for a match and every hand:
 * asset handshake, peer wait screen, dice, initial deal, and ready report.
 */
export function useMatchOpening({
  matchId,
  view,
  assetsLoaded,
  onCommand,
}: MatchOpeningOptions): MatchOpeningState {
  const assetsReadySeats = view?.assets_ready_seats ?? null;
  const allAssetsReady =
    view == null ||
    assetsReadySeats == null ||
    assetsReadySeats.length >= view.players.length;
  const localAssetsReady =
    view == null ||
    assetsReadySeats == null ||
    assetsReadySeats.includes(view.observer_seat);
  const assetsTimedOut = Boolean(view?.terminated_by_asset_timeout);
  const [phase, setPhase] = useState<OpeningPhase>("dice");
  const [sequenceComplete, setSequenceComplete] = useState(false);
  const waitedForPeers = useRef(false);

  useEffect(() => {
    if (!assetsLoaded || localAssetsReady) return;
    onCommand("game.assets_ready");
    const timer = window.setInterval(
      () => onCommand("game.assets_ready"),
      2_000,
    );
    return () => window.clearInterval(timer);
  }, [assetsLoaded, localAssetsReady, onCommand]);

  if (
    view &&
    assetsReadySeats != null &&
    assetsReadySeats.length < view.players.length
  ) {
    waitedForPeers.current = true;
  }

  useSceneWaitingPeers(
    view && assetsLoaded && !assetsTimedOut && waitedForPeers.current
      ? {
          ready: assetsReadySeats?.length ?? view.players.length,
          total: view.players.length,
        }
      : null,
  );

  useEffect(() => {
    if (!view || view.id !== matchId || !allAssetsReady) return;
    const readySeats = view.opening_ready_seats ?? [];
    const alreadyPlaying =
      readySeats.includes(view.observer_seat) ||
      handAlreadyMoved(view);
    if (alreadyPlaying) {
      setPhase("waiting");
      setSequenceComplete(true);
      onCommand("game.ready_for_hand", { hand_index: view.hand_index });
      return;
    }

    setSequenceComplete(false);
    setPhase("dice");
    const startDealing = window.setTimeout(
      () => setPhase("deal"),
      OPENING_DICE_MS,
    );
    const finishDealing = window.setTimeout(() => {
      setPhase("waiting");
      setSequenceComplete(true);
      onCommand("game.ready_for_hand", { hand_index: view.hand_index });
    }, OPENING_DICE_MS + openingDealDuration(view.players.length));

    const soundTimers = [0, 4, 8, 12].map((tileIndex) => {
      const delay =
        OPENING_DICE_MS +
        openingDealArrival(
          tileIndex,
          view.observer_seat,
          view.progress.dealer,
          view.players.length,
        ) +
        TILE_STAND_UP_MS;
      return window.setTimeout(() => playSfx(NEWROUND_PAIS_SFX), delay);
    });

    return () => {
      window.clearTimeout(startDealing);
      window.clearTimeout(finishDealing);
      soundTimers.forEach(window.clearTimeout);
    };
    // The hand identity, not every view update, owns this animation.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [allAssetsReady, matchId, view?.hand_index, view?.id]);

  useEffect(() => {
    if (!view || !sequenceComplete) return;
    const readySeats = view.opening_ready_seats;
    if (
      readySeats == null ||
      readySeats.length >= view.players.length ||
      handAlreadyMoved(view)
    ) {
      setPhase("play");
    }
  }, [sequenceComplete, view, view?.opening_ready_seats]);

  return { phase, allAssetsReady, assetsTimedOut };
}

function handAlreadyMoved(view: MatchView): boolean {
  return (
    view.players.some(
      (player) => player.discards.length > 0 || player.melds.length > 0,
    ) ||
    view.hand_settlement != null ||
    view.result != null
  );
}
