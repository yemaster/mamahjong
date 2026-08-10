import type { EndReason, MatchView, WaitingTileView } from "../types";

/** Every announcement the table can shout above a player's hand. */
export type CallKind =
  | "chi"
  | "pon"
  | "kan"
  | "riichi"
  | "ron"
  | "tsumo"
  | "tenpai"
  | "noten"
  | "draw";

export interface CallBannerItem {
  id: string;
  /** null pins the banner to the centre of the table instead of a seat. */
  seat: number | null;
  kind: CallKind;
  /** 覆盖默认文字，流局用它显示具体的流局原因。 */
  label?: string;
  /** 流局摊牌时听牌者具体听的牌。 */
  waits?: WaitingTileView[];
  /**
   * 需要长时间挂在牌桌上的横幅（流局的听牌/不听）在这里给出停留毫秒数，
   * 让淡出正好卡在被移除的那一刻，而不是弹完就消失。
   */
  holdMs?: number;
}

/**
 * 副露只会喊这三种。单独取出来是因为它们同时也是 `VoiceKind` 的三个值，
 * 播报和语音可以直接共用同一个 kind，不必再映射一次。
 */
export type MeldCallKind = Extract<CallKind, "chi" | "pon" | "kan">;

export const CALL_LABELS: Record<CallKind, string> = {
  chi: "吃",
  pon: "碰",
  kan: "杠",
  riichi: "立直",
  ron: "荣和",
  tsumo: "自摸",
  tenpai: "听牌",
  noten: "不听",
  draw: "流局",
};

/**
 * 流局横幅要说清楚是怎么流的：牌摸完了是荒牌流局，其余四种是途中流局。
 * 和牌结束不会走到这里，兜底仍然写「流局」。
 */
const DRAW_REASON_LABELS: Record<EndReason, string> = {
  exhaustive_draw: "荒牌流局",
  nine_terminals: "九种九牌",
  four_winds: "四风连打",
  four_kans: "四杠散了",
  four_riichi: "四家立直",
  tsumo: "流局",
  ron: "流局",
};

export function drawReasonLabel(reason: EndReason): string {
  return DRAW_REASON_LABELS[reason] ?? CALL_LABELS.draw;
}

/**
 * Melds that appeared since the previous view.
 *
 * A brand new meld announces its own call; a 碰 upgraded into 加杠 keeps the
 * same meld id but changes kind, so it announces 杠 a second time.
 */
export function detectMeldCalls(
  view: MatchView,
  previous: MatchView,
): { seat: number; kind: MeldCallKind }[] {
  const calls: { seat: number; kind: MeldCallKind }[] = [];
  for (const player of view.players) {
    const before = previous.players.find(
      (candidate) => candidate.seat === player.seat,
    );
    const previousKinds = new Map(
      (before?.melds ?? []).map((meld) => [meld.id, meld.kind]),
    );
    for (const meld of player.melds) {
      if (previousKinds.get(meld.id) === meld.kind) continue;
      calls.push({
        seat: player.seat,
        kind:
          meld.kind === "chi" ? "chi" : meld.kind === "pon" ? "pon" : "kan",
      });
    }
  }
  return calls;
}

/**
 * Seats that declared 立直 since the previous view. The declaration shows up
 * as `pending` and only turns into `established` once the discard passes, so
 * the banner fires on the first move away from `none`.
 */
export function detectRiichiCalls(
  view: MatchView,
  previous: MatchView,
): number[] {
  return view.players
    .filter((player) => {
      if (player.riichi_status === "none") return false;
      const before = previous.players.find(
        (candidate) => candidate.seat === player.seat,
      );
      return before != null && before.riichi_status === "none";
    })
    .map((player) => player.seat);
}

/**
 * 这一巡立直算不算两立直。
 *
 * 两立直的条件是「第一巡、且此前无人鸣牌」。客户端手上没有巡目这个数，用桌面
 * 上看得见的两件事去卡：谁都还没有副露，并且全场打出的牌不超过一人一张——第
 * 一圈最多就是最后一家宣言时的每人一张，转到第二圈必然超。
 *
 * 声音在宣言当场就要出，那时候还没有役种可查，只能这么判。
 */
export function isDoubleRiichiTurn(view: MatchView): boolean {
  if (view.players.some((player) => player.melds.length > 0)) {
    return false;
  }
  const discards = view.players.reduce(
    (total, player) => total + player.discards.length,
    0,
  );
  return discards <= view.players.length;
}

/** 流局 opens hands one at a time, starting with the dealer. */
export function drawRevealOrder(view: MatchView): number[] {
  const seats = view.players.map((player) => player.seat).sort((a, b) => a - b);
  const start = seats.indexOf(view.progress.dealer);
  if (start < 0) return seats;
  return [...seats.slice(start), ...seats.slice(0, start)];
}
