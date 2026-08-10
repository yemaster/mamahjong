import type { HandSettlementView } from "../types";
import type { HandRecord } from "./recordTypes";

/** 一局的结算：牌桌演出要的那一份，外加它够不够开面板。 */
export interface ReplaySettlement {
  view: HandSettlementView;
  /**
   * 番符役种齐不齐。
   *
   * 齐了才开结算面板；不齐就只播和牌演出——喊声、砸牌、摊手牌照旧，最后不升面板。
   */
  detailed: boolean;
}

/**
 * 把一局的结算摊成牌桌认的 `HandSettlementView`。
 *
 * 重演不另写结算面板：对局里那块 `HandSettlement` 认的就是这个结构，牌谱补齐同样
 * 的字段就能把和牌那一屏原样播一遍（见 `docs/match-record-replay.md`）。
 *
 * 两条口径：
 * - **流局不结算。** 用户要看的是和牌那一下，流局既不摊听牌也不开面板，直接翻下一局。
 * - **旧牌谱降级到只演不算。** `winner_scores` 是后来才加的字段，归档一旦写完就冻住，
 *   所以这个字段之前打的每一局都补不回来。缺了不能整段不播：谁和的、怎么和的
 *   （`winners` / `reason` / `from`）事件日志里本来就有，演出照演，只是番符那块空着，
 *   面板不开。
 */
export function replayHandSettlement(hand: HandRecord): ReplaySettlement | null {
  if (hand.winners.length === 0) return null;
  const scores = hand.winner_scores ?? [];
  const detailed = scores.length > 0;

  const view: HandSettlementView = {
    reason: hand.reason,
    tenpai_seats: hand.tenpai,
    point_deltas: hand.point_deltas,
    points_before: hand.points_before,
    points_after: hand.points_after,
    /*
     * 降级那条路只有座位号是真的：番符役种一律留空。反正 `detailed` 为假时面板不开，
     * 这些数字不会上屏——动画认的只有座位和是自摸还是荣和。
     */
    winners: detailed
      ? scores.map((score) => ({
          seat: score.seat,
          han: score.han,
          fu: score.fu,
          yakuman_multiplier: score.yakuman_multiplier,
          limit: score.limit,
          points: score.points,
          dealer: score.dealer,
          yaku: score.yaku,
        }))
      : hand.winners.map((seat) => ({
          seat,
          han: 0,
          fu: 0,
          yakuman_multiplier: 0,
          limit: "",
          points: 0,
          dealer: seat === hand.dealer,
          yaku: [],
        })),
    /*
     * 播完动画、开确认窗口这些是对局里的握手，重演没有对手要等：一律写成空，
     * 面板什么时候亮由 `useReplaySettlement` 的计时器说了算。
     */
    played_seats: [],
    confirm_remaining_ms: null,
    confirmed_seats: [],
    from_seat: hand.from,
    ura_dora_indicators: hand.ura_dora_indicators ?? [],
  };

  return { view, detailed };
}
