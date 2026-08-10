import { describe, expect, it } from "vitest";
import type { MeldView, TileView } from "../types";
import {
  countTiles,
  isWinningHand,
  seatWaits,
  tileCodeAt,
  tileIndex,
  waitRemainingCount,
  waitsFromCounts,
} from "./waits";

/** 把 "123m" 这样的简写摊成牌码数组，省得每个用例手写一长串。 */
function hand(...groups: string[]): string[] {
  const codes: string[] = [];
  for (const group of groups) {
    const suit = group[group.length - 1] ?? "";
    for (const rank of group.slice(0, -1)) codes.push(`${rank}${suit}`);
  }
  return codes;
}

/** 牌码数组包成 `TileView`，id 只要互不相同就行。 */
function tiles(codes: string[]): TileView[] {
  return codes.map((code, index) => ({ id: index + 1, code }));
}

function waitsOf(codes: string[], meldCount = 0): string[] {
  return waitsFromCounts(countTiles(codes), meldCount).sort();
}

describe("tileIndex / tileCodeAt", () => {
  it("三色和字牌来回转得回去", () => {
    for (let index = 0; index < 34; index += 1) {
      expect(tileIndex(tileCodeAt(index))).toBe(index);
    }
  });

  it("赤五当普通五", () => {
    expect(tileIndex("0m")).toBe(tileIndex("5m"));
    expect(tileIndex("0p")).toBe(tileIndex("5p"));
    expect(tileIndex("0s")).toBe(tileIndex("5s"));
  });

  it("认不出来的牌返回 -1", () => {
    expect(tileIndex("8z")).toBe(-1);
    expect(tileIndex("back")).toBe(-1);
  });
});

describe("waitsFromCounts", () => {
  it("两面：456m 缺 78s 的两头", () => {
    expect(waitsOf(hand("123m", "456m", "789m", "11p", "78s"))).toEqual([
      "6s",
      "9s",
    ]);
  });

  it("单骑：孤张听自己", () => {
    expect(waitsOf(hand("123m", "456m", "789m", "123p", "5s"))).toEqual(["5s"]);
  });

  it("嵌张：13s 只听中间", () => {
    expect(waitsOf(hand("123m", "456m", "789m", "11p", "13s"))).toEqual(["2s"]);
  });

  it("九莲宝灯是九面听", () => {
    expect(waitsOf(hand("1112345678999m"))).toEqual([
      "1m",
      "2m",
      "3m",
      "4m",
      "5m",
      "6m",
      "7m",
      "8m",
      "9m",
    ]);
  });

  it("七对子听单张", () => {
    expect(waitsOf(hand("112233m", "4455p", "66s", "9s"))).toEqual(["9s"]);
  });

  it("国士十三面", () => {
    expect(waitsOf(hand("19m", "19p", "19s", "1234567z"))).toEqual([
      "1m",
      "1p",
      "1s",
      "1z",
      "2z",
      "3z",
      "4z",
      "5z",
      "6z",
      "7z",
      "9m",
      "9p",
      "9s",
    ]);
  });

  it("国士单骑：多的那张幺九只听缺的那一种", () => {
    expect(waitsOf(hand("119m", "19p", "19s", "123456z"))).toEqual(["7z"]);
  });

  it("副露之后只按剩下的张数判", () => {
    /* 两副露 + 七张暗手，听 3m/6m 的两面。 */
    expect(waitsOf(hand("45m", "789p", "11s"), 2)).toEqual(["3m", "6m"]);
  });

  it("七对子和国士在副露之后一律不成立", () => {
    expect(waitsOf(hand("112233m", "4455p", "66s", "9s"), 1)).toEqual([]);
  });

  it("没听的形返回空", () => {
    expect(waitsOf(hand("123m", "456m", "789m", "13p", "58s"))).toEqual([]);
  });

  it("自己手上四张的牌不算听", () => {
    /* 1111m + 一堆，第五张 1m 不存在。 */
    expect(waitsOf(hand("1111m", "234m", "567m", "99p", "9p"))).not.toContain(
      "1m",
    );
  });
});

describe("isWinningHand", () => {
  it("十四张标准型", () => {
    expect(
      isWinningHand(countTiles(hand("123m", "456m", "789m", "123p", "99s")), 0),
    ).toBe(true);
  });

  it("差一张不算和", () => {
    expect(
      isWinningHand(countTiles(hand("123m", "456m", "789m", "123p", "98s")), 0),
    ).toBe(false);
  });
});

describe("seatWaits", () => {
  const melds: MeldView[] = [];

  it("十三张形直接判", () => {
    expect(
      seatWaits(tiles(hand("123m", "456m", "789m", "11p", "78s")), melds).sort(),
    ).toEqual(["6s", "9s"]);
  });

  it("十四张形先摘掉刚摸的那张", () => {
    const full = tiles(hand("123m", "456m", "789m", "11p", "789s"));
    const drawn = full[full.length - 1]!;
    expect(seatWaits(full, melds, drawn.id).sort()).toEqual(["6s", "9s"]);
  });

  it("摸牌 id 对不上就退回摘最后一张", () => {
    const full = tiles(hand("123m", "456m", "789m", "11p", "789s"));
    expect(seatWaits(full, melds, 9999).sort()).toEqual(["6s", "9s"]);
  });

  it("张数不成形（刚被鸣走还没打）返回空", () => {
    expect(seatWaits(tiles(hand("123m", "456m")), melds)).toEqual([]);
  });
});

describe("waitRemainingCount", () => {
  it("一种四张，减掉桌上看得见的", () => {
    const visible = new Map([
      ["6s", 1],
      ["9s", 4],
    ]);
    expect(waitRemainingCount(["6s", "9s"], visible)).toBe(3);
  });

  it("看得见的比四张还多也不会算成负数", () => {
    expect(waitRemainingCount(["6s"], new Map([["6s", 9]]))).toBe(0);
  });

  it("赤五和普通五数同一格", () => {
    expect(waitRemainingCount(["0m"], new Map([["5m", 2]]))).toBe(2);
  });
});
