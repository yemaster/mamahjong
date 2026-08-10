import type { MatchResult, MatchView } from "../../types";

export interface ResultRow {
  seat: number;
  rank: number;
  nickname: string;
  avatarPath: string | null;
  /** 是不是自家。高亮的是自家那条，不是一位那条——名次牌已经写着谁是一位了。 */
  isSelf: boolean;
  /** 终局点数。 */
  points: number;
  /** 马点：服务端按十分之一存，这里换成展示用的一位小数。 */
  score: number;
  /** 整场点数增减。立直麻将不送这项，为 `null`。 */
  pointDelta: number | null;
  /**
   * 整场杠点增减。冲击麻将独有的第二本账，起始值是 0，所以结余就是增减本身；
   * 立直麻将没有这本账，为 `null`。
   */
  kanPointDelta: number | null;
}

/** 名次条之间错开的时间。 */
export const ROW_ENTER_STAGGER_MS = 120;
/** 一条名次条飞进来用的时间。 */
export const ROW_ENTER_MS = 420;

/**
 * 终局名次列表，一位在最前。
 *
 * 点数以 `final_points` 为准：名次是结算当时算出来的，点数在那之后还会因为
 * 场供归属再变一次，两处对不上时以最终点数为准。
 */
export function resultRows(view: MatchView, result: MatchResult): ResultRow[] {
  const players = new Map(view.players.map((player) => [player.seat, player]));
  return [...result.placements]
    .sort((left, right) => left.rank - right.rank)
    .flatMap((placement) => {
      const player = players.get(placement.seat);
      if (!player) return [];
      return [
        {
          seat: placement.seat,
          rank: placement.rank,
          nickname: player.nickname,
          avatarPath: player.avatar_path,
          isSelf: placement.seat === view.observer_seat,
          points: result.final_points[placement.seat] ?? placement.points,
          score: placement.score_tenths / 10,
          pointDelta: result.point_deltas?.[placement.seat] ?? null,
          kanPointDelta: result.kan_points?.[placement.seat] ?? null,
        },
      ];
    });
}

/** 马点写法：正数带加号，一律保留一位小数，零不带符号。 */
export function formatScore(score: number): string {
  const magnitude = Math.abs(score).toFixed(1);
  if (score > 0) return `+${magnitude}`;
  if (score < 0) return `-${magnitude}`;
  return magnitude;
}

/** 增减写法：整数，正数带加号，零写成 `±0` 好和「没这项」区分开。 */
export function formatDelta(delta: number): string {
  if (delta > 0) return `+${delta}`;
  if (delta < 0) return `-${Math.abs(delta)}`;
  return "±0";
}

/** 增减的冷暖：加是暖色、减是冷色、不动是灰的，和局中点数变动那块同一套。 */
export function deltaTone(delta: number): " is-plus" | " is-minus" | " is-even" {
  if (delta > 0) return " is-plus";
  if (delta < 0) return " is-minus";
  return " is-even";
}

/**
 * 名次条登场的先后：从末位往一位数，冠军那条最后落下。
 *
 * 结果先揭晓再看谁赢就没有悬念了，末位先出场才是把冠军留到最后。
 */
export function rowEnterDelayMs(rank: number, rowCount: number): number {
  return Math.max(0, rowCount - rank) * ROW_ENTER_STAGGER_MS;
}

/** 名次条全部落定的时刻，确认按钮等到这时候才淡进来。 */
export function rowsSettledMs(rowCount: number): number {
  return rowEnterDelayMs(1, rowCount) + ROW_ENTER_MS;
}
