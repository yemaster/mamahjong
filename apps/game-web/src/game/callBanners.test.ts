import { describe, expect, it } from "vitest";
import type { MatchView } from "../types";
import {
  detectMeldCalls,
  detectRiichiCalls,
  drawRevealOrder,
  isDoubleRiichiTurn,
} from "./callBanners";

const view = (
  players: {
    seat: number;
    melds?: { id: number; kind: string }[];
    riichi_status?: string;
    /** 这一家已经打出去几张，只有判两立直时才在意。 */
    discards?: number;
  }[],
  dealer = 0,
) =>
  ({
    players: players.map((player) => ({
      seat: player.seat,
      melds: player.melds ?? [],
      riichi_status: player.riichi_status ?? "none",
      discards: Array.from({ length: player.discards ?? 0 }, () => ({})),
    })),
    progress: { dealer },
  }) as unknown as MatchView;

describe("鸣牌播报", () => {
  it("新副露按类型播报", () => {
    const before = view([{ seat: 0 }, { seat: 1 }]);
    const after = view([
      { seat: 0, melds: [{ id: 1, kind: "chi" }] },
      { seat: 1, melds: [{ id: 2, kind: "pon" }] },
    ]);
    expect(detectMeldCalls(after, before)).toEqual([
      { seat: 0, kind: "chi" },
      { seat: 1, kind: "pon" },
    ]);
  });

  it("三种杠都播报杠", () => {
    for (const kind of ["open_kan", "concealed_kan", "added_kan"]) {
      const before = view([{ seat: 2 }]);
      const after = view([{ seat: 2, melds: [{ id: 5, kind }] }]);
      expect(detectMeldCalls(after, before)).toEqual([
        { seat: 2, kind: "kan" },
      ]);
    }
  });

  it("碰升级成加杠会再播一次", () => {
    const before = view([{ seat: 3, melds: [{ id: 4, kind: "pon" }] }]);
    const after = view([{ seat: 3, melds: [{ id: 4, kind: "added_kan" }] }]);
    expect(detectMeldCalls(after, before)).toEqual([{ seat: 3, kind: "kan" }]);
  });

  it("副露没变化时不播报", () => {
    const same = view([{ seat: 0, melds: [{ id: 1, kind: "chi" }] }]);
    expect(detectMeldCalls(same, same)).toEqual([]);
  });
});

describe("立直播报", () => {
  it("刚宣言立直时播报", () => {
    const before = view([{ seat: 0 }, { seat: 1 }]);
    const after = view([
      { seat: 0, riichi_status: "pending" },
      { seat: 1 },
    ]);
    expect(detectRiichiCalls(after, before)).toEqual([0]);
  });

  it("立直生效后不再重复播报", () => {
    const before = view([{ seat: 0, riichi_status: "pending" }]);
    const after = view([{ seat: 0, riichi_status: "established" }]);
    expect(detectRiichiCalls(after, before)).toEqual([]);
  });
});

describe("两立直判定", () => {
  it("第一巡各家至多一张，算两立直", () => {
    /* 最后一家宣言时，全场正好每人一张。 */
    const table = view([
      { seat: 0, discards: 1 },
      { seat: 1, discards: 1 },
      { seat: 2, discards: 1 },
      { seat: 3, discards: 1 },
    ]);
    expect(isDoubleRiichiTurn(table)).toBe(true);
  });

  it("开局第一家立直算两立直", () => {
    const table = view([
      { seat: 0 },
      { seat: 1 },
      { seat: 2 },
      { seat: 3 },
    ]);
    expect(isDoubleRiichiTurn(table)).toBe(true);
  });

  it("转到第二巡就不算", () => {
    const table = view([
      { seat: 0, discards: 2 },
      { seat: 1, discards: 1 },
      { seat: 2, discards: 1 },
      { seat: 3, discards: 1 },
    ]);
    expect(isDoubleRiichiTurn(table)).toBe(false);
  });

  it("有人鸣过牌就不算，哪怕还在第一巡", () => {
    const table = view([
      { seat: 0, discards: 1 },
      { seat: 1, discards: 1, melds: [{ id: 1, kind: "pon" }] },
      { seat: 2 },
      { seat: 3 },
    ]);
    expect(isDoubleRiichiTurn(table)).toBe(false);
  });

  it("三麻按三家算一巡", () => {
    const sanma = view([
      { seat: 0, discards: 1 },
      { seat: 1, discards: 1 },
      { seat: 2, discards: 1 },
    ]);
    expect(isDoubleRiichiTurn(sanma)).toBe(true);
    const later = view([
      { seat: 0, discards: 2 },
      { seat: 1, discards: 1 },
      { seat: 2, discards: 1 },
    ]);
    expect(isDoubleRiichiTurn(later)).toBe(false);
  });
});

describe("流局摊牌顺序", () => {
  it("从庄家开始绕一圈", () => {
    const table = view([{ seat: 0 }, { seat: 1 }, { seat: 2 }, { seat: 3 }], 2);
    expect(drawRevealOrder(table)).toEqual([2, 3, 0, 1]);
  });

  it("庄家在座位表里缺失时按座次", () => {
    const table = view([{ seat: 0 }, { seat: 1 }, { seat: 2 }], 3);
    expect(drawRevealOrder(table)).toEqual([0, 1, 2]);
  });
});
