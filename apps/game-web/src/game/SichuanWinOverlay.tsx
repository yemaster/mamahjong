import { useEffect, useMemo, useState } from "react";
import type { MatchView, SichuanWinView } from "../types";
import { playSfx, SCORE_CHANGE_SFX } from "../audio/sfx";
import {
  DELTA_COUNT_MS,
  PointChangeCard,
  cardBeats,
} from "./pointChangeCard";

/** 点数卡退场后，留给牌桌盖牌/亮胡张的时长。 */
const WIN_REVEAL_MS = 900;
/** 数字滚完后停一拍，和杠分动画一致。 */
const HOLD_MS = 900;
/** 点数浮层退场时长，和 `pointChangeVeilOut` 对齐。 */
const EXIT_MS = 280;

interface SichuanWinOverlayProps {
  view: MatchView;
  win: SichuanWinView;
  onReveal: () => void;
  onFinished: () => void;
}

export function SichuanWinOverlay({
  view,
  win,
  onReveal,
  onFinished,
}: SichuanWinOverlayProps) {
  const [slammed, setSlammed] = useState(false);
  const [leaving, setLeaving] = useState(false);

  /* 四川的杠与胡都直接改同一份比赛点数。视图里的 player.points 已经是本次
     胡牌后的值，因此用事件增量倒推出动画起点。 */
  const cards = useMemo(
    () =>
      view.players.map((player, index) => {
        const delta = win.deltas[player.seat] ?? 0;
        const after = player.points;
        return {
          key: player.user_id,
          avatarPath: player.avatar_path,
          nickname: player.nickname,
          isSelf: player.seat === view.observer_seat,
          before: after - delta,
          after,
          delta,
          index,
        };
      }),
    [view.observer_seat, view.players, win.deltas],
  );

  const slamAt = useMemo(() => {
    const beats = cards.flatMap((card) =>
      card.delta === 0 ? [] : [cardBeats(card.index).countAt],
    );
    return beats.length > 0 ? Math.min(...beats) : null;
  }, [cards]);

  const revealAt = useMemo(() => {
    const beats = cards.flatMap((card) =>
      card.delta === 0 ? [] : [cardBeats(card.index).riseAt],
    );
    return beats.length > 0 ? Math.min(...beats) : null;
  }, [cards]);

  const pointEndAt = useMemo(() => {
    const beats = cards.flatMap((card) =>
      card.delta === 0 ? [] : [cardBeats(card.index).countAt],
    );
    const last = beats.length > 0 ? Math.max(...beats) : 0;
    const visualEnd = last + DELTA_COUNT_MS + HOLD_MS;
    const soundEnd = revealAt != null ? revealAt + 1200 : 0;
    return Math.max(visualEnd, soundEnd);
  }, [cards, revealAt]);

  useEffect(() => {
    if (slamAt == null) return;
    const timer = window.setTimeout(() => setSlammed(true), slamAt);
    return () => window.clearTimeout(timer);
  }, [slamAt]);

  useEffect(() => {
    if (revealAt == null) return;
    const timers: number[] = [];
    for (let index = 0; index < 6; index += 1) {
      timers.push(
        window.setTimeout(
          () => playSfx(SCORE_CHANGE_SFX),
          revealAt + index * 200,
        ),
      );
    }
    return () => timers.forEach(window.clearTimeout);
  }, [revealAt]);

  useEffect(() => {
    /* 先完整展示并滚动点数；浮层退掉后再让牌桌盖牌/标红胡张。 */
    const leaveTimer = window.setTimeout(() => setLeaving(true), pointEndAt);
    const revealTimer = window.setTimeout(onReveal, pointEndAt + EXIT_MS);
    const doneTimer = window.setTimeout(
      onFinished,
      pointEndAt + EXIT_MS + WIN_REVEAL_MS,
    );
    return () => {
      window.clearTimeout(leaveTimer);
      window.clearTimeout(revealTimer);
      window.clearTimeout(doneTimer);
    };
  }, [onFinished, onReveal, pointEndAt, win.id]);

  return (
    <div
      className={`match-point-change is-kan is-sichuan-win${
        leaving ? " is-leaving" : ""
      }`}
      aria-label="胡牌点数变动"
      role="status"
    >
      <div
        className={`match-point-change__cards${slammed ? " is-slammed" : ""}`}
      >
        {cards.map((card) => (
          <PointChangeCard
            key={card.key}
            avatarPath={card.avatarPath}
            nickname={card.nickname}
            isSelf={card.isSelf}
            before={card.before}
            after={card.after}
            delta={card.delta}
            index={card.index}
          />
        ))}
      </div>
    </div>
  );
}
