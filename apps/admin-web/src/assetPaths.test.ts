import { describe, expect, it } from "vitest";
import { formatAssetSize, parentAssetPath, publicAssetUrl } from "./assetPaths";

describe("managed asset paths", () => {
  it("encodes every public URL segment without losing folders", () => {
    expect(publicAssetUrl("角色/春 日.png")).toBe("/user-assets/%E8%A7%92%E8%89%B2/%E6%98%A5%20%E6%97%A5.png");
  });

  it("supports navigation and readable sizes", () => {
    expect(parentAssetPath("characters/outfits/default")).toBe("characters/outfits");
    expect(formatAssetSize(1536)).toBe("1.5 KB");
  });
});
