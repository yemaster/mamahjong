import { describe, expect, it } from "vitest";
import {
  moveTileInOrder,
  reconcileManualTileOrder,
} from "./PlayerHand2D";

describe("手动理牌顺序", () => {
  it("保留玩家调整后的顺序，并把新摸牌加在最右侧", () => {
    expect(
      reconcileManualTileOrder([3, 1, 2], [1, 2, 3, 4]),
    ).toEqual([3, 1, 2, 4]);
  });

  it("打出牌后只移除该牌，不移动其余手牌", () => {
    expect(reconcileManualTileOrder([3, 1, 2, 4], [1, 3, 4])).toEqual([
      3, 1, 4,
    ]);
  });

  it("拖动牌会改变本地显示顺序", () => {
    expect(moveTileInOrder([1, 2, 3, 4], 4, 2)).toEqual([1, 4, 2, 3]);
  });
});
