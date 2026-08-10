/**
 * 三维牌桌对外的门面。
 *
 * 内部按职责拆成了若干模块：
 * - `constants` 尺寸常量，`geometry` 摆位算术，`animation` 动画曲线与时长
 * - `handView` 从对局视图里挑出要画的东西（理牌、副露摆法、听牌摊牌范围）
 * - `doraShine` 宝牌那道扫光，`tileHighlight` 拿起手牌时点亮桌上的同种牌，
 *   `impact` 牌砸到桌面那一下的扬灰与镜头颤动
 * - `tileMesh` 造一张牌，`tableSurface`/`wall`/`hands`/`discards`/`melds`/
 *   `centerConsole`/`dice` 各自往场景里搭一块
 * - `runtime` 渲染器与那一条动画循环，`scene` 把上面这些按视图拼起来
 */
export { GameTable } from "./GameTable";
export type {
  TableCameraConfig,
  TableRuntime,
  TileAnimation,
} from "./types";
export {
  DEFAULT_TABLECLOTH_ASSET,
  DEFAULT_TILE_SCALE,
  TILE_LENGTH,
  TILE_STAND_UP_MS,
  TILE_WIDTH_RATIO,
} from "./constants";
export {
  billboardHandTilt,
  coveredHandTilt,
  MELD_PUSH_MS,
  meldPushSource,
  openingDealArrival,
  openingDealDuration,
  openingDealOrder,
  openingDealStep,
  settlementFallEase,
  settlementHandShift,
  settlementHandTilt,
  standUpEase,
  standingHandTilt,
  TSUMO_THROW_MS,
  tsumoThrowArc,
  tsumoThrowEase,
} from "./animation";
export { DRAW_MOVE_MS, HAND_COLLAPSE_MS } from "./hands";
export { opponentHandLayout } from "./opponentHandMotion";
export {
  DICE_SETTLE_MS,
  DORA_FLIP_DELAY_MS,
  DORA_FLIP_MS,
  OPENING_DICE_MS,
} from "./dice";
export {
  addedKanTilePosition,
  discardGridPosition,
  discardNaturalRotation,
  doraWallTileIndex,
  handPosition,
  meldTilePosition,
  orthographicCameraBounds,
  rinshanWallSlot,
  screenRectAnchor,
  tableCameraLayout,
  tableLayoutZones,
  tableRelativeSeat,
  wallBreakSlot,
  wallTileOrigin,
  wallTileQuaternion,
} from "./geometry";
export {
  canLocalPlayerDiscard,
  countCompletedKans,
  isJokerTile,
  meldDisplayTiles,
  playerCompletedKan,
  playerIsHoldingDrawnTile,
  riverDiscardEntries,
  settlementCoveringSeats,
  sortHandForDisplay,
  winningTileIndex,
} from "./handView";
export type { WallLayout } from "./wallLayout";
export { impactWallLayout, riichiWallLayout } from "./wallLayout";
