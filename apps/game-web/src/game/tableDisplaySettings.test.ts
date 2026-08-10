import { beforeEach, describe, expect, it } from "vitest";
import {
  defaultTablePerspectiveSettings,
  loadTablePerspectiveSettings,
  saveTablePerspectiveSettings,
  tableCameraConfigFromSettings,
} from "./tableDisplaySettings";

describe("个人牌桌设置", () => {
  beforeEach(() => localStorage.clear());

  it("不同用户分别保存自己的透视镜头", () => {
    saveTablePerspectiveSettings("用户甲", {
      ...defaultTablePerspectiveSettings,
      height: 61,
    });
    saveTablePerspectiveSettings("用户乙", {
      ...defaultTablePerspectiveSettings,
      height: 42,
    });

    expect(loadTablePerspectiveSettings("用户甲").height).toBe(61);
    expect(loadTablePerspectiveSettings("用户乙").height).toBe(42);
  });

  it("只生成透视摄像机并按夹角计算前后位置", () => {
    const camera = tableCameraConfigFromSettings(
      defaultTablePerspectiveSettings,
    );
    expect(camera.mode).toBe("perspective");
    expect(camera.y).toBe(21);
    expect(camera.z).toBeCloseTo(18.2);
  });
});
