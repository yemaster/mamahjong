import { describe, expect, it } from "vitest";
import type { MatchPlayerView, MatchView } from "../types";
import {
  normalizeTileCode,
  tileRemaining,
  visibleTileCounts,
} from "./tileCounts";

function view(overrides: Partial<MatchView> = {}): MatchView {
  return {
    dora_indicators: [],
    observer_seat: 0,
    players: [],
    ...overrides,
  } as unknown as MatchView;
}

function player(
  seat: number,
  overrides: Record<string, unknown> = {},
): MatchPlayerView {
  return {
    seat,
    concealed_tiles: null,
    melds: [],
    discards: [],
    ...overrides,
  } as unknown as MatchPlayerView;
}

const tile = (id: number, code: string) => ({ id, code });
const discard = (id: number, code: string, claimedBy: number | null = null) => ({
  tile: tile(id, code),
  tsumogiri: false,
  riichi_declared: false,
  claimed_by: claimedBy,
});

describe("剩余枚数", () => {
  it("赤宝牌算成同数字的普通牌", () => {
    expect(normalizeTileCode("0m")).toBe("5m");
    expect(normalizeTileCode("5p")).toBe("5p");
    expect(normalizeTileCode("1z")).toBe("1z");
  });

  it("没露过面的牌剩四枚", () => {
    expect(tileRemaining(visibleTileCounts(view()), "3s")).toBe(4);
  });

  it("自己手上、牌河、副露和宝牌指示牌都算已经看见", () => {
    const counts = visibleTileCounts(
      view({
        dora_indicators: [tile(1, "1m")],
        players: [
          player(0, { concealed_tiles: [tile(2, "1m"), tile(3, "9p")] }),
          player(1, { discards: [discard(4, "1m")] }),
          player(2, {
            melds: [
              { id: 1, kind: "pon", tiles: [tile(5, "1m")], called_from: 1 },
            ],
          }),
        ],
      }),
    );
    expect(tileRemaining(counts, "1m")).toBe(0);
    expect(tileRemaining(counts, "9p")).toBe(3);
  });

  it("别家的暗手看不见，不算进去", () => {
    const counts = visibleTileCounts(
      view({
        players: [
          player(0, { concealed_tiles: [tile(1, "4s")] }),
          player(1, { concealed_tiles: [tile(2, "4s"), tile(3, "4s")] }),
        ],
      }),
    );
    expect(tileRemaining(counts, "4s")).toBe(3);
  });

  it("被鸣走的弃牌只算副露那一份，不重复计数", () => {
    const counts = visibleTileCounts(
      view({
        players: [
          player(0, { discards: [discard(1, "7p", 1)] }),
          player(1, {
            melds: [
              {
                id: 1,
                kind: "pon",
                tiles: [tile(1, "7p"), tile(2, "7p"), tile(3, "7p")],
                called_from: 0,
              },
            ],
          }),
        ],
      }),
    );
    expect(tileRemaining(counts, "7p")).toBe(1);
  });

  it("赤五和普通五合并计数", () => {
    const counts = visibleTileCounts(
      view({
        players: [
          player(0, { concealed_tiles: [tile(1, "0m"), tile(2, "5m")] }),
        ],
      }),
    );
    expect(tileRemaining(counts, "5m")).toBe(2);
    expect(tileRemaining(counts, "0m")).toBe(2);
  });
});
