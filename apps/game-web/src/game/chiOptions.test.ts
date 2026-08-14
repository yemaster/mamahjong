import { describe, expect, it } from "vitest";
import type { ReactionOption, TileView } from "../types";
import { chiCommandName, chiOptions } from "./chiOptions";

function hand(codes: string[]): TileView[] {
  return codes.map((code, index) => ({ id: 100 + index, code }));
}

function chi(...ids: number[]): ReactionOption {
  return { kind: "chi", tile_ids: [ids[0]!, ids[1]!] };
}

describe("chiOptions", () => {
  it("红五和普通的五各算一种方案", () => {
    /* 手上 3s 3s 5s 0s 6s，上家打 4s。 */
    const tiles = hand(["3s", "3s", "5s", "0s", "6s"]);
    const options = chiOptions(
      [
        chi(100, 102),
        chi(100, 103),
        chi(102, 104),
        chi(103, 104),
      ],
      tiles,
    );
    expect(options.map((option) => option.key)).toEqual([
      "3s+5s",
      "3s+0s",
      "5s+6s",
      "0s+6s",
    ]);
  });

  it("牌码一样的方案只列一次", () => {
    const tiles = hand(["3s", "3s", "5s"]);
    const options = chiOptions([chi(100, 102), chi(101, 102)], tiles);
    expect(options).toHaveLength(1);
    /* 留下的那条得是能直接发出去的：id 必须是手上真有的那两张。 */
    expect(options[0]!.tileIds).toEqual([100, 102]);
  });

  it("一组之内按牌序排，红五跟在同数字的普通牌后面", () => {
    const tiles = hand(["6s", "0s"]);
    const options = chiOptions([chi(101, 100)], tiles);
    expect(options[0]!.tiles.map((tile) => tile.code)).toEqual(["0s", "6s"]);
  });

  it("只认吃，碰和杠不进来", () => {
    const tiles = hand(["3s", "5s"]);
    const options = chiOptions(
      [
        { kind: "ron" },
        { kind: "pon", tile_ids: [100, 101] },
        { kind: "open_kan", tile_ids: [100, 101, 102] },
      ],
      tiles,
    );
    expect(options).toEqual([]);
  });

  it("手上认不出那两张牌就不画这一条", () => {
    const options = chiOptions([chi(900, 901)], hand(["3s", "5s"]));
    expect(options).toEqual([]);
  });

  it("多方案选择完成后按麻将种类发送对应吃牌指令", () => {
    expect(chiCommandName({ variant_kind: "impact" })).toBe("impact.chi");
    expect(chiCommandName({ variant_kind: "riichi" })).toBe("riichi.chi");
  });
});
