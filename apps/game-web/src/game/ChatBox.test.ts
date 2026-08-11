import { describe, expect, it } from "vitest";
import { chatPopoverPlacement } from "./ChatBox";

describe("表情浮层定位", () => {
  it("优先贴在输入框上方且右侧对齐", () => {
    expect(
      chatPopoverPlacement(
        { left: 1200, top: 700, width: 208, height: 40 },
        { width: 280, height: 228 },
        { width: 1600, height: 900 },
      ),
    ).toEqual({ left: 1128, top: 464 });
  });

  it("上方不足时放到下方且不遮盖输入框", () => {
    expect(
      chatPopoverPlacement(
        { left: 300, top: 20, width: 208, height: 40 },
        { width: 280, height: 228 },
        { width: 1600, height: 900 },
      ),
    ).toEqual({ left: 228, top: 68 });
  });

  it("聊天框靠近舞台边缘时把浮层夹在屏幕内", () => {
    expect(
      chatPopoverPlacement(
        { left: 4, top: 400, width: 208, height: 40 },
        { width: 280, height: 228 },
        { width: 1600, height: 900 },
      ),
    ).toEqual({ left: 0, top: 164 });
    expect(
      chatPopoverPlacement(
        { left: 1530, top: 400, width: 70, height: 40 },
        { width: 280, height: 228 },
        { width: 1600, height: 900 },
      ).left,
    ).toBe(1320);
  });
});
