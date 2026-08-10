export const DOM_STAGE_WIDTH = 1600;
export const DOM_STAGE_HEIGHT = 900;

/** 只计算一个统一倍率，横纵坐标和尺寸永远一起缩放。 */
export function fixedDomStageScale(width: number, height: number): number {
  if (!(width > 0) || !(height > 0)) return 1;
  return Math.min(width / DOM_STAGE_WIDTH, height / DOM_STAGE_HEIGHT);
}

export interface FixedDomStageFrame {
  scale: number;
  width: number;
  height: number;
  left: number;
  top: number;
}

/** 把缩放后的屏幕像素换回舞台设计像素，供点击和拖动共用。 */
export function visualPixelsToStage(
  visualPixels: number,
  scale: number,
): number {
  return scale > 0 ? visualPixels / scale : visualPixels;
}

/** 返回缩放后的实际占用尺寸及严格居中的左上角坐标。 */
export function fixedDomStageFrame(
  viewportWidth: number,
  viewportHeight: number,
): FixedDomStageFrame {
  const scale = fixedDomStageScale(viewportWidth, viewportHeight);
  const width = DOM_STAGE_WIDTH * scale;
  const height = DOM_STAGE_HEIGHT * scale;
  return {
    scale,
    width,
    height,
    left: (viewportWidth - width) / 2,
    top: (viewportHeight - height) / 2,
  };
}
