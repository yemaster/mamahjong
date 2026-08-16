import { describe, expect, it } from "vitest";
import {
  advanceTileCode,
  DEV_HAND_KEYS,
  DEV_HAND_SIZE,
  validTileCodes,
} from "./devMode";
import { tileCodes } from "./tileAssets";

describe("devMode", () => {
  it("14 个键正好对应最多 14 张暗手", () => {
    expect(DEV_HAND_KEYS).toHaveLength(DEV_HAND_SIZE);
    expect(DEV_HAND_KEYS).toBe("qwertyuiopasdf");
  });

  it("advanceTileCode 顺着给定牌码循环推进", () => {
    expect(advanceTileCode("1m", tileCodes)).toBe("2m");
    expect(advanceTileCode("9m", tileCodes)).toBe("0p");
    expect(advanceTileCode("9s", tileCodes)).toBe("1z");
    expect(advanceTileCode("7z", tileCodes)).toBe("0m");
  });

  it("赤宝牌带 r 后缀时先归一成 0m 再推进", () => {
    expect(advanceTileCode("5mr", tileCodes)).toBe("1m");
  });

  it("认不出的牌码原样返回", () => {
    expect(advanceTileCode("?", tileCodes)).toBe("?");
  });

  it("冲击麻将没有赤牌，跳过 0m/0p/0s", () => {
    const valid = validTileCodes("impact", false);
    expect(valid).not.toContain("0m");
    expect(valid).not.toContain("0p");
    expect(valid).not.toContain("0s");
    expect(advanceTileCode("9m", valid)).toBe("1p");
    expect(advanceTileCode("9s", valid)).toBe("1z");
  });

  it("三麻跳过 2m..8m 和 0m", () => {
    const valid = validTileCodes("riichi", true);
    expect(valid).not.toContain("0m");
    for (let rank = 2; rank <= 8; rank += 1) {
      expect(valid).not.toContain(`${rank}m`);
    }
    expect(valid).toContain("1m");
    expect(valid).toContain("9m");
    expect(valid).toContain("0p");
    expect(valid).toContain("0s");
    expect(advanceTileCode("1m", valid)).toBe("9m");
    expect(advanceTileCode("9m", valid)).toBe("0p");
  });

  it("四麻保留全量牌码", () => {
    expect(validTileCodes("riichi", false)).toEqual([...tileCodes]);
  });
});
