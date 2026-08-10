import { useEffect, useMemo, useState } from "react";
import type { MatchView } from "../types";
import { playSfx, SCORE_CHANGE_SFX } from "../audio/sfx";
import { PointChangeCard, cardBeats } from "./pointChangeCard";

type HandSettlement = NonNullable<MatchView["hand_settlement"]>;

interface PointChangeOverlayProps {
  view: MatchView;
  /** Shows the confirm button once the point animation has finished. */
  confirmReady?: boolean;
  confirmed?: boolean;
  secondsRemaining?: number;
  onConfirm?: () => void;
}

export function PointChangeOverlay(props: PointChangeOverlayProps) {
  const settlement = props.view.hand_settlement;
  if (!settlement) return null;
  return <PointChangeBoard {...props} settlement={settlement} />;
}

function PointChangeBoard({
  view,
  settlement,
  confirmReady = false,
  confirmed = false,
  secondsRemaining,
  onConfirm,
}: PointChangeOverlayProps & { settlement: HandSettlement }) {
  /*
   * 点数是整段结算的落锤：分数砸回原尺寸的同时整块面板被撞得一颤。四家只震这一下,
   * 时刻取最先开始滚动的那一家——每家各震一次就成了持续晃动。
   */
  const slamAt = useMemo(() => {
    const beats = view.players.flatMap((player, index) => {
      const before = settlement.points_before[player.seat] ?? player.points;
      const after = settlement.points_after[player.seat] ?? player.points;
      return after === before ? [] : [cardBeats(index).countAt];
    });
    return beats.length > 0 ? Math.min(...beats) : null;
  }, [settlement, view.players]);

  /* 变化点数开始往分数位置飞的那一刻起播计分音效，播 6 次，间隔 200ms。 */
  const revealAt = useMemo(() => {
    const beats = view.players.flatMap((player, index) => {
      const before = settlement.points_before[player.seat] ?? player.points;
      const after = settlement.points_after[player.seat] ?? player.points;
      return after === before ? [] : [cardBeats(index).riseAt];
    });
    return beats.length > 0 ? Math.min(...beats) : null;
  }, [settlement, view.players]);

  const [slammed, setSlammed] = useState(false);

  useEffect(() => {
    if (slamAt == null) return;
    const timer = window.setTimeout(() => setSlammed(true), slamAt);
    return () => window.clearTimeout(timer);
  }, [slamAt]);

  useEffect(() => {
    if (revealAt == null) return;
    const timers: number[] = [];
    for (let i = 0; i < 6; i++) {
      timers.push(
        window.setTimeout(() => playSfx(SCORE_CHANGE_SFX), revealAt + i * 200),
      );
    }
    return () => timers.forEach(window.clearTimeout);
  }, [revealAt]);

  return (
    <div className="match-point-change" aria-label="点数变动">
      {confirmReady && onConfirm && (
        <button
          className="match-point-change__confirm"
          type="button"
          disabled={confirmed}
          onClick={onConfirm}
        >
          {confirmed
            ? "已确认"
            : secondsRemaining != null
              ? `确认 ${secondsRemaining}`
              : "确认"}
        </button>
      )}
      <div
        className={`match-point-change__cards${slammed ? " is-slammed" : ""}`}
      >
        {view.players.map((player, index) => {
          const before =
            settlement.points_before[player.seat] ?? player.points;
          const after =
            settlement.points_after[player.seat] ?? player.points;

          return (
            <PointChangeCard
              key={player.user_id}
              avatarPath={player.avatar_path}
              nickname={player.nickname}
              isSelf={player.seat === view.observer_seat}
              before={before}
              after={after}
              delta={after - before}
              index={index}
            />
          );
        })}
      </div>
    </div>
  );
}
