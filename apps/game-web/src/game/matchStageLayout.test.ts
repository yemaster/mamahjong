import { describe, expect, it } from "vitest";
import { MATCH_STAGE_HEIGHT, matchStageScale } from "./matchStageLayout";
import { tableCameraLayout } from "./table/geometry";

/** 界面舞台和三维镜头都只跟窗口高度走，否则窗口一变形两边就对不上。 */
describe("界面舞台与镜头同步缩放", () => {
  it("设计高度下缩放为 1", () => {
    expect(matchStageScale(MATCH_STAGE_HEIGHT)).toBe(1);
  });

  it("按高度等比缩放", () => {
    expect(matchStageScale(1800)).toBeCloseTo(2);
    expect(matchStageScale(450)).toBeCloseTo(0.5);
  });

  it("高度未知时不缩放", () => {
    expect(matchStageScale(0)).toBe(1);
  });

  it("纵向视野角不随画幅变化", () => {
    const base = tableCameraLayout(16 / 9).fov;
    for (const aspect of [21 / 9, 4 / 3, 1, 3 / 4]) {
      expect(tableCameraLayout(aspect).fov).toBe(base);
    }
  });

  /* 三维画面的屏幕缩放正比于 视口高 / tan(纵向视野角一半)，视野角既然固定，
     它就和舞台缩放一样只看高度。 */
  it("三维缩放与舞台缩放同步", () => {
    const screenScale = (width: number, height: number) => {
      const halfFov = (tableCameraLayout(width / height).fov / 2) * (Math.PI / 180);
      return height / Math.tan(halfFov);
    };
    const base = screenScale(1600, MATCH_STAGE_HEIGHT);
    for (const [width, height] of [
      [2560, 1440],
      [3440, 1440],
      [1280, 720],
      [900, 900],
      [800, 1200],
    ] as const) {
      expect(screenScale(width, height) / base).toBeCloseTo(
        matchStageScale(height),
        6,
      );
    }
  });
});
