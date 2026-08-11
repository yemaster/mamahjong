import { describe, expect, it } from "vitest";
import { createAssetBundle, isTableclothInput, parseAssetBundle, upsertAssetItems } from "./assetTransfer";
import { vi } from "vitest";

const tablecloth = {
  id: "green",
  name: "绿色桌布",
  texture_path: "/assets/green.png",
  enabled: true,
  is_default: false,
};

describe("asset transfer", () => {
  it("creates and parses a versioned bundle", () => {
    const bundle = createAssetBundle("tablecloths", [tablecloth], new Date("2026-08-11T00:00:00Z"));
    expect(parseAssetBundle(JSON.stringify(bundle), "tablecloths", isTableclothInput)).toEqual([tablecloth]);
  });

  it("rejects another resource kind", () => {
    const bundle = createAssetBundle("music", [tablecloth]);
    expect(() => parseAssetBundle(JSON.stringify(bundle), "tablecloths", isTableclothInput)).toThrow("文件类型或版本不正确");
  });

  it("rejects invalid item data", () => {
    const bundle = createAssetBundle("tablecloths", [{ id: "broken" }]);
    expect(() => parseAssetBundle(JSON.stringify(bundle), "tablecloths", isTableclothInput)).toThrow("文件中的数据格式不正确");
  });

  it("updates existing ids and creates new ids", async () => {
    const create = vi.fn().mockResolvedValue(undefined);
    const update = vi.fn().mockResolvedValue(undefined);
    const added = { ...tablecloth, id: "blue" };
    await upsertAssetItems([tablecloth, added], ["green"], create, update);

    expect(update).toHaveBeenCalledWith(tablecloth);
    expect(create).toHaveBeenCalledWith(added);
  });
});
