import { describe, expect, it } from "vitest";
import { tablePreviewView } from "../../game/tablePreviewData";
import type { MatchResult } from "../../types";
import {
  formatDelta,
  formatScore,
  resultRows,
  rowEnterDelayMs,
  rowsSettledMs,
} from "./resultRows";

function placement(
  seat: number,
  rank: number,
  points: number,
  scoreTenths: number,
) {
  return {
    seat,
    rank,
    points,
    uma_tenths: 0,
    oka_tenths: 0,
    score_tenths: scoreTenths,
  };
}

const result: MatchResult = {
  end_reason: "south_end",
  hand_count: 8,
  final_points: [48300, 33400, 21300, 7000],
  placements: [
    placement(2, 3, 21300, -187),
    placement(0, 1, 48300, 563),
    placement(3, 4, 7000, -460),
    placement(1, 2, 33400, 84),
  ],
  unclaimed_riichi_sticks_awarded: 0,
};

describe("终局名次", () => {
  it("按名次从一位排到末位", () => {
    expect(resultRows(tablePreviewView, result).map((row) => row.rank)).toEqual([
      1, 2, 3, 4,
    ]);
  });

  it("点数以最终点数为准，马点换成一位小数", () => {
    const rows = resultRows(tablePreviewView, result);
    expect(rows[0]).toMatchObject({ seat: 0, points: 48300, score: 56.3 });
    expect(rows[3]).toMatchObject({ seat: 3, points: 7000, score: -46 });
  });

  it("高亮跟着自家走，不是跟着一位走", () => {
    const rows = resultRows(
      { ...tablePreviewView, observer_seat: 2 },
      result,
    );
    expect(rows.filter((row) => row.isSelf).map((row) => row.seat)).toEqual([2]);
    expect(rows[0]).toMatchObject({ rank: 1, isSelf: false });
  });

  it("冲击麻将带上点数增减和杠点增减，立直麻将两项都空着", () => {
    const impact = resultRows(tablePreviewView, {
      ...result,
      point_deltas: [12, -5, 0, -7],
      kan_points: [-3, 8, 0, -5],
    });
    expect(impact[0]).toMatchObject({ pointDelta: 12, kanPointDelta: -3 });
    expect(impact[2]).toMatchObject({ pointDelta: 0, kanPointDelta: 0 });

    const riichi = resultRows(tablePreviewView, result);
    expect(riichi[0]).toMatchObject({ pointDelta: null, kanPointDelta: null });
  });

  it("增减写成整数，正数带加号，零写成 ±0", () => {
    expect(formatDelta(12)).toBe("+12");
    expect(formatDelta(-7)).toBe("-7");
    expect(formatDelta(0)).toBe("±0");
  });

  it("名次里有查不到的座位就跳过，不摆空条", () => {
    const rows = resultRows(tablePreviewView, {
      ...result,
      placements: [...result.placements, placement(9, 5, 0, 0)],
    });
    expect(rows).toHaveLength(4);
  });

  it("马点正数带加号，一律保留一位小数", () => {
    expect(formatScore(56.3)).toBe("+56.3");
    expect(formatScore(8)).toBe("+8.0");
    expect(formatScore(-18.7)).toBe("-18.7");
    expect(formatScore(0)).toBe("0.0");
  });

  it("末位先登场，冠军那条最后落下", () => {
    expect(rowEnterDelayMs(4, 4)).toBe(0);
    expect(rowEnterDelayMs(1, 4)).toBeGreaterThan(rowEnterDelayMs(2, 4));
  });

  it("整段进场不超过一秒", () => {
    expect(rowsSettledMs(4)).toBeLessThanOrEqual(1000);
  });
});
