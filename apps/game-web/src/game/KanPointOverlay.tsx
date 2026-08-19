import { useEffect, useMemo, useState } from "react";
import type { KanPointsKind, KanPointsView, MatchView } from "../types";
import { playSfx, SCORE_CHANGE_SFX } from "../audio/sfx";
import { DELTA_COUNT_MS, PointChangeCard, cardBeats } from "./pointChangeCard";

/*
 * 冲击麻将的杠点是和点数分开的一本账，一杠一动，一局能动好几次。演出直接照抄和
 * 了结算那块点数变动浮层——同一批卡片、同一套节拍，只是数字换成杠点、标题写明是
 * 哪种杠。区别在于它不值得停下来让人按一次「确认」：亮出来、把数字滚完、自己消
 * 失。代价是这几秒里的输入得挡住，不然玩家会在没看清杠点走向的情况下就把下一张
 * 牌打出去；`onFinished` 就是给上层解锁用的，服务端也等着这个信号才摸岭上牌。
 */

const KAN_LABELS: Record<KanPointsKind, string> = {
  open_kan: "明杠",
  concealed_kan: "暗杠",
  added_kan: "加杠",
  indicator_pon: "指示牌碰",
  indicator_concealed: "指示牌暗杠",
  first_round_repeat_discard: "第一巡连打",
  four_identical_discards: "四张相同牌",
  four_consecutive_discards: "四张连续相同牌",
  three_indicator_discards: "三张指示牌",
  three_consecutive_indicator: "三张连续指示牌",
  chankan: "抢杠",
};

/** 数字滚完之后停的那一拍，留给人读完自己是加是减。 */
const HOLD_MS = 900;
/** 退场动画时长，和 `pointChangeVeilOut` 那条动画对齐。 */
const EXIT_MS = 280;

interface KanPointOverlayProps {
  view: MatchView;
  kan: KanPointsView;
  /** 播完了。上层照这个信号收掉浮层并放开操作。 */
  onFinished: () => void;
}

export function KanPointOverlay({
  view,
  kan,
  onFinished,
}: KanPointOverlayProps) {
  const [leaving, setLeaving] = useState(false);
  const [slammed, setSlammed] = useState(false);

  /*
   * 视图送来的 `kan_points` 已经是变动之后的值（杠完当场就变，不等结算），变动前
   * 的数字倒推回去。四家都列出来，没动的那两家跟着一起亮，和点数变动那块一个样。
   */
  const cards = useMemo(
    () =>
      view.players.map((player, index) => {
        const delta = kan.deltas[player.seat] ?? 0;
        // 四川麻将的杠点并入本局即时点账，没有单独的 kan_points 字段。
        const after = player.kan_points ?? player.points;
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
    [kan.deltas, view.observer_seat, view.players],
  );

  /* 最先开始滚的那一家落锤，整块面板跟着颤一下。 */
  const slamAt = useMemo(() => {
    const beats = cards.flatMap((card) =>
      card.delta === 0 ? [] : [cardBeats(card.index).countAt],
    );
    return beats.length > 0 ? Math.min(...beats) : null;
  }, [cards]);

  /* 变化点数开始往杠点位置飞的那一刻起播计分音效，播 6 次，间隔 200ms。 */
  const revealAt = useMemo(() => {
    const beats = cards.flatMap((card) =>
      card.delta === 0 ? [] : [cardBeats(card.index).riseAt],
    );
    return beats.length > 0 ? Math.min(...beats) : null;
  }, [cards]);

  /* 最后一家滚完 + 音效序列播完，取较晚者。 */
  const finishAt = useMemo(() => {
    const beats = cards.flatMap((card) =>
      card.delta === 0 ? [] : [cardBeats(card.index).countAt],
    );
    const last = beats.length > 0 ? Math.max(...beats) : 0;
    const visualEnd = last + DELTA_COUNT_MS + HOLD_MS;
    // 音效序列：revealAt 起播 6 次 × 200ms 间隔 = +1000ms，再多给 200ms 让最后一声播完
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
    for (let i = 0; i < 6; i++) {
      timers.push(
        window.setTimeout(() => playSfx(SCORE_CHANGE_SFX), revealAt + i * 200),
      );
    }
    return () => timers.forEach(window.clearTimeout);
  }, [revealAt]);

  useEffect(() => {
    const exitTimer = window.setTimeout(() => setLeaving(true), finishAt);
    const doneTimer = window.setTimeout(onFinished, finishAt + EXIT_MS);
    return () => {
      window.clearTimeout(exitTimer);
      window.clearTimeout(doneTimer);
    };
  }, [finishAt, onFinished]);

  const actor = view.players.find((player) => player.seat === kan.seat);

  return (
    <div
      className={`match-point-change is-kan${leaving ? " is-leaving" : ""}`}
      aria-label={view.variant_kind === "sichuan" ? "点数变动" : "杠点变动"}
      role="status"
    >
      <div className="match-point-change__banner">
        <strong>{KAN_LABELS[kan.kind]}</strong>
        {actor && <span>{actor.nickname}</span>}
      </div>
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
            caption={view.variant_kind === "sichuan" ? undefined : "杠点"}
          />
        ))}
      </div>
    </div>
  );
}
