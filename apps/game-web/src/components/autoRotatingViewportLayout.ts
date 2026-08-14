import {
  DOM_STAGE_HEIGHT,
  DOM_STAGE_WIDTH,
  fixedDomStageScale,
} from "./fixedDomStageLayout";

export interface AutoRotatingViewportFrame {
  /** 应用在自己的排版坐标系里能使用的宽高。 */
  width: number;
  height: number;
  /** true 时整个应用顺时针旋转 90° 后显示。 */
  rotated: boolean;
  /** 当前方向下 1600×900 舞台能取得的最大等比缩放。 */
  scale: number;
}

/**
 * 比较原方向和顺时针旋转 90° 后的舞台大小，选择严格更大的一个。
 *
 * 两个候选都按「完整放下 1600×900，并尽可能占满一条边」计算；相等时不旋转，
 * 避免接近正方形的窗口因为浮点误差来回翻转。
 */
export function autoRotatingViewportFrame(
  viewportWidth: number,
  viewportHeight: number,
): AutoRotatingViewportFrame {
  if (!(viewportWidth > 0) || !(viewportHeight > 0)) {
    return {
      width: DOM_STAGE_WIDTH,
      height: DOM_STAGE_HEIGHT,
      rotated: false,
      scale: 1,
    };
  }

  const unrotatedScale = fixedDomStageScale(viewportWidth, viewportHeight);
  const rotatedScale = fixedDomStageScale(viewportHeight, viewportWidth);
  const rotated = rotatedScale > unrotatedScale;

  return {
    width: rotated ? viewportHeight : viewportWidth,
    height: rotated ? viewportWidth : viewportHeight,
    rotated,
    scale: rotated ? rotatedScale : unrotatedScale,
  };
}
