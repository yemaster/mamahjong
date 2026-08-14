import { describe, expect, it } from "vitest";
import {
  yakuEntryTabs,
  yakuReferenceEntries,
  yakuValueTags,
} from "./yakuReferenceData";

describe("役种番数标签", () => {
  it("役满倍数保持为单独标签", () => {
    expect(yakuValueTags("役满")).toEqual(["役满"]);
    expect(yakuValueTags("双倍役满")).toEqual(["双倍役满"]);
  });

  it("大四喜归入双倍役满", () => {
    const daisuushi = yakuReferenceEntries.find((entry) => entry.name === "大四喜");

    expect(daisuushi?.value).toBe("双倍役满");
    expect(yakuEntryTabs(daisuushi!)).toEqual(["双倍役满"]);
  });

  it("每个役种都有番数或役满标签", () => {
    expect(
      yakuReferenceEntries.every((entry) =>
        yakuValueTags(entry.value).every((tag) =>
          /番|役满/.test(tag),
        ),
      ),
    ).toBe(true);
  });

  it("副露减番役只进入门前番数页签", () => {
    const sanshoku = yakuReferenceEntries.find(
      (entry) => entry.name === "三色同顺",
    )!;
    const chinitsu = yakuReferenceEntries.find(
      (entry) => entry.name === "清一色",
    )!;
    expect(yakuEntryTabs(sanshoku)).toEqual(["2番"]);
    expect(yakuEntryTabs(chinitsu)).toEqual(["6番"]);
  });

  it("副露示例按暗手、副露、和牌排列并保留两个间隔", () => {
    const openEntries = yakuReferenceEntries.filter(
      (entry) => entry.openReduction,
    );
    expect(openEntries).toHaveLength(6);
    for (const entry of openEntries) {
      expect(entry.meldGroups).toEqual([
        { start: 10, length: 3, calledTileIndex: 10 },
      ]);
      expect(entry.winningTileIndex).toBe(13);
      expect(entry.tiles).toHaveLength(14);
    }
  });

  it("门前清限定役明确标记且示例没有副露", () => {
    for (const name of [
      "门前清自摸和",
      "立直",
      "一发",
      "平和",
      "一杯口",
      "两立直",
      "七对子",
      "二杯口",
      "国士无双",
      "四暗刻",
      "九莲宝灯",
    ]) {
      const entry = yakuReferenceEntries.find(
        (candidate) => candidate.name === name,
      );
      expect(entry?.menzenRequired, name).toBe(true);
      expect(entry?.meldGroups, name).toBeUndefined();
    }
  });

  it("杠子役和岭上开花使用对应数量的杠副露", () => {
    const rinshan = yakuReferenceEntries.find(
      (entry) => entry.name === "岭上开花",
    )!;
    const threeKans = yakuReferenceEntries.find(
      (entry) => entry.name === "三杠子",
    )!;
    const fourKans = yakuReferenceEntries.find(
      (entry) => entry.name === "四杠子",
    )!;
    expect(rinshan.meldGroups).toHaveLength(1);
    expect(threeKans.meldGroups).toHaveLength(3);
    expect(fourKans.meldGroups).toHaveLength(4);
    expect(
      [rinshan, threeKans, fourKans].every((entry) =>
        entry.meldGroups?.every((meld) => meld.length === 4),
      ),
    ).toBe(true);
  });

  it("风牌、三元牌和大三元使用副露突出役牌组合", () => {
    for (const name of [
      "自风",
      "场风",
      "役牌白",
      "役牌发",
      "役牌中",
    ]) {
      expect(
        yakuReferenceEntries.find((entry) => entry.name === name)
          ?.meldGroups,
        name,
      ).toHaveLength(1);
    }
    expect(
      yakuReferenceEntries.find((entry) => entry.name === "大三元")
        ?.meldGroups,
    ).toHaveLength(3);
  });

  it("平和把两面听的和牌单独放在最右侧", () => {
    const pinfu = yakuReferenceEntries.find(
      (entry) => entry.name === "平和",
    );
    expect(pinfu?.tiles.slice(9, 11)).toEqual(["6s", "7s"]);
    expect(pinfu?.winningTileIndex).toBe(13);
    expect(pinfu?.tiles.at(-1)).toBe("8s");
  });
});
