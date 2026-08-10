/**
 * 对局界面的设计基准高度。
 *
 * 二维界面按「高度固定 900、宽度横向撑满窗口」的设计坐标书写，再整体按高度
 * 缩放。三维镜头的纵向视野角同样固定，所以两边的缩放比例一致，窗口变宽只是
 * 左右各多露出一点桌面，界面和牌桌的相对关系不变。
 */
export const MATCH_STAGE_HEIGHT = 900;

/** 舞台缩放比例：只看高度，横向由舞台自己撑满窗口。 */
export function matchStageScale(height: number): number {
  if (!(height > 0)) return 1;
  return height / MATCH_STAGE_HEIGHT;
}
