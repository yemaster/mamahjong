import * as THREE from "three";
import {
  DEFAULT_TILE_SCALE,
  HAND_DISTANCE,
  MELD_DISTANCE,
  MELD_RIGHT_EDGE,
  MELD_TILE_LENGTH,
  OPPONENT_TILE_LENGTH,
  RIVER_TILE_GAP,
  RIVER_TILE_LENGTH,
  TABLE_CLOTH_SIDE,
  TABLE_IMAGE_SIDE,
  TABLE_TILE_LENGTH,
  TILE_DEPTH_RATIO,
  TILE_LENGTH,
  TILE_WIDTH_RATIO,
  TOTAL_WALL_TILES,
  WALL_DISTANCE,
  WALL_STACKS_PER_SIDE,
  WALL_STACK_GAP,
  WALL_TILE_DEPTH,
  WALL_TILE_LENGTH,
} from "./constants";

/**
 * 座位换算：把绝对座次换成「相对本家的第几家」。
 *
 * 三人场也按四个方位摆桌，所以座位数按 4 取模。
 */
export function tableRelativeSeat(
  seat: number,
  observer: number,
  count: number,
): number {
  const tableSeatCount = count === 3 ? 4 : count;
  return (seat + tableSeatCount - observer) % tableSeatCount;
}

/** 把本家坐标系里的点绕桌心转到对应座位上。 */
export function rotateAroundTable(
  position: THREE.Vector3,
  quarterTurns: number,
): void {
  position.applyAxisAngle(
    new THREE.Vector3(0, 1, 0),
    quarterTurns * (Math.PI / 2),
  );
}

export function handPosition(
  relative: number,
  _count: number,
  index: number,
  isSelf: boolean,
  drawnGap = 0,
  laidOutOnTable = false,
  widthRatio = TILE_WIDTH_RATIO,
  tileScale = DEFAULT_TILE_SCALE,
  outwardShift = 0,
): THREE.Vector3 {
  const tileLength =
    (isSelf ? TILE_LENGTH : OPPONENT_TILE_LENGTH) * tileScale;
  const width = tileLength * widthRatio;
  const step = width + 0.025 * tileScale;
  const fixedRackSpan = 12 * step;
  const position = new THREE.Vector3(
    -fixedRackSpan / 2 + index * step + drawnGap * tileScale,
    laidOutOnTable
      ? tileLength * TILE_DEPTH_RATIO / 2 + 0.09
      : isSelf
        ? 0.45
        : 0.41,
    HAND_DISTANCE + outwardShift,
  );
  rotateAroundTable(position, relative);
  return position;
}

export function handQuaternion(
  relative: number,
  _isSelf: boolean,
): THREE.Quaternion {
  return new THREE.Quaternion().setFromAxisAngle(
    new THREE.Vector3(0, 1, 0),
    relative * (Math.PI / 2),
  );
}

export function discardGridPosition(
  index: number,
  widthRatio = TILE_WIDTH_RATIO,
  tileScale = DEFAULT_TILE_SCALE,
  /** 末行序号（0 起）。默认 2 = 三行；冲击麻将用 3 = 四行，摸的牌多。 */
  maxRow = 2,
): { x: number; z: number } {
  const row = Math.min(maxRow, Math.floor(index / 6));
  const column = row < maxRow ? index % 6 : index - maxRow * 6;
  const scaledWidth = RIVER_TILE_LENGTH * widthRatio * tileScale;
  const scaledLength = RIVER_TILE_LENGTH * tileScale;
  return {
    x: (column - 2.5) * (scaledWidth + RIVER_TILE_GAP),
    z: 1.72 + row * (scaledLength + RIVER_TILE_GAP),
  };
}

export function discardNaturalRotation(
  seat: number,
  discardIndex: number,
): number {
  let hash =
    Math.imul(seat + 1, -1640531527) ^
    Math.imul(discardIndex + 1, -2048144789);
  hash ^= hash >>> 16;
  const normalized = (hash >>> 0) / 0x1_0000_0000;
  const sign = (hash & 1) === 0 ? -1 : 1;
  const degrees =
    normalized < 0.06
      ? 4 + (normalized / 0.06) * 1.5
      : 0.4 + ((normalized - 0.06) / 0.94) * 1.6;
  return THREE.MathUtils.degToRad(sign * degrees);
}

export function meldTilePosition(
  relative: number,
  cursor: number,
  rotated: boolean,
  widthRatio = TILE_WIDTH_RATIO,
  tileScale = DEFAULT_TILE_SCALE,
): THREE.Vector3 {
  const meldLength = MELD_TILE_LENGTH * tileScale;
  const meldWidth = meldLength * widthRatio;
  const footprint = rotated ? meldLength : meldWidth;
  const bottomAlignment = rotated
    ? (meldLength - meldWidth) / 2
    : 0;
  const position = new THREE.Vector3(
    MELD_RIGHT_EDGE - cursor - footprint / 2,
    (meldLength * TILE_DEPTH_RATIO) / 2 + 0.09,
    MELD_DISTANCE + bottomAlignment,
  );
  rotateAroundTable(position, relative);
  return position;
}

export function addedKanTilePosition(
  calledTilePosition: THREE.Vector3,
  relative: number,
  widthRatio = TILE_WIDTH_RATIO,
  tileScale = DEFAULT_TILE_SCALE,
): THREE.Vector3 {
  const offset = new THREE.Vector3(
    0,
    0,
    -(MELD_TILE_LENGTH * widthRatio * tileScale + 0.006),
  );
  rotateAroundTable(offset, relative);
  return calledTilePosition.clone().add(offset);
}

/** 牌桌永远按四面摆墙，三麻也一样，只是有一面前面没人坐。 */
const WALL_SIDES = TOTAL_WALL_TILES / (WALL_STACKS_PER_SIDE * 2);

/**
 * 摸牌序号 → 牌墙上的物理位置。
 *
 * 桌上的牌墙是这么摆的：每家面前 17 墩，从这家的左手边数到右手边；四面按
 * 「自家 → 下家 → 对家 → 上家」逆时针接起来（自家右端接下家左端）。
 *
 * 摸牌却是从开口往**左手边**取，取完一家的墙接着取上家的墙，正好是沿着上面
 * 那条物理排布倒着走。所以整套代码里的 `slot` 一律是「第几张摸的牌」，到了
 * 要算坐标的时候才在这里翻回物理位置：序号每加 2 就往左挪一墩，挪满 17 墩就
 * 换到上家那面墙的右端。同一墩里先上面那张、后下面那张。
 */
function wallTilePlacement(slot: number): {
  side: number;
  stack: number;
  layer: number;
} {
  const normalizedSlot =
    ((slot % TOTAL_WALL_TILES) + TOTAL_WALL_TILES) % TOTAL_WALL_TILES;
  const stackAlongPath = Math.floor(normalizedSlot / 2);
  const sidesPassed = Math.floor(stackAlongPath / WALL_STACKS_PER_SIDE);
  return {
    side: (WALL_SIDES - sidesPassed) % WALL_SIDES,
    stack:
      WALL_STACKS_PER_SIDE - 1 - (stackAlongPath % WALL_STACKS_PER_SIDE),
    layer: 1 - (normalizedSlot % 2),
  };
}

/** 一张牌摆在牌墙第 `side` 面、从左数第 `stack` 墩、第 `layer` 层的世界坐标。 */
function wallPlacementOrigin(
  placement: { side: number; stack: number; layer: number },
  widthRatio: number,
  tileScale: number,
): THREE.Vector3 {
  const wallTileWidth = WALL_TILE_LENGTH * widthRatio * tileScale;
  const wallTileDepth = WALL_TILE_DEPTH * tileScale;
  const origin = new THREE.Vector3(
    (placement.stack - 8) * (wallTileWidth + WALL_STACK_GAP),
    0.12 +
      wallTileDepth / 2 +
      placement.layer * (wallTileDepth + 0.015),
    WALL_DISTANCE,
  );
  rotateAroundTable(origin, placement.side);
  return origin;
}

function wallPlacementQuaternion(placement: { side: number }): THREE.Quaternion {
  return new THREE.Quaternion().setFromAxisAngle(
    new THREE.Vector3(0, 1, 0),
    placement.side * (Math.PI / 2),
  );
}

export function wallTileOrigin(
  slot: number,
  widthRatio = TILE_WIDTH_RATIO,
  tileScale = DEFAULT_TILE_SCALE,
): THREE.Vector3 {
  return wallPlacementOrigin(wallTilePlacement(slot), widthRatio, tileScale);
}

export function wallTileQuaternion(slot: number): THREE.Quaternion {
  return wallPlacementQuaternion(wallTilePlacement(slot));
}

/**
 * 冲击麻将的牌墙位置换算。
 *
 * 立直那边的 `slot` 是「第几张摸的牌」，所以要倒着换算回物理位置；冲击麻将的
 * `slot` 直接就是物理位置——`impactWallLayout` 自己排摸牌顺序，这里不用管顺序。
 *
 * 槽位 = 相对座次 * 34 + 墩号 * 2 + 层，墩号从这一家的**左手边**数到右手边，
 * 同一墩里 0 是上层、1 是下层。相对座次 0=自家(屏幕下) 1=下家(右) 2=对家(上)
 * 3=上家(左)，和 `rotateAroundTable` 的四分之一圈一一对应，中间不能再翻面：
 * 一翻就成了镜像，顺时针摸牌看着会变成逆时针。
 */
function impactWallTilePlacement(slot: number): {
  side: number;
  stack: number;
  layer: number;
} {
  const normalizedSlot =
    ((slot % TOTAL_WALL_TILES) + TOTAL_WALL_TILES) % TOTAL_WALL_TILES;
  const stackIndex = Math.floor(normalizedSlot / 2);
  return {
    side: Math.floor(stackIndex / WALL_STACKS_PER_SIDE) % WALL_SIDES,
    stack: stackIndex % WALL_STACKS_PER_SIDE,
    layer: 1 - (normalizedSlot % 2),
  };
}

export function impactWallTileOrigin(
  slot: number,
  widthRatio = TILE_WIDTH_RATIO,
  tileScale = DEFAULT_TILE_SCALE,
): THREE.Vector3 {
  return wallPlacementOrigin(
    impactWallTilePlacement(slot),
    widthRatio,
    tileScale,
  );
}

export function impactWallTileQuaternion(slot: number): THREE.Quaternion {
  return wallPlacementQuaternion(impactWallTilePlacement(slot));
}

/**
 * 开门位置：第一张被摸走的牌的序号。
 *
 * 骰子点数从**庄家**数起（庄家算 1，往下家方向数），数到谁就拆谁面前那道墙；
 * 再从那家牌墙的右端留下点数那么多墩，紧挨着的下一墩就是开口。例如掷出 7 点，
 * 拆庄家对家的墙，留 7 墩，从第 8 墩开始摸。
 *
 * `dealerRelative` 是庄家相对本家的座次（`tableRelativeSeat` 的结果）。
 */
export function wallBreakSlot(
  dealerRelative: number,
  diceTotal: number,
): number {
  const breakSide = (dealerRelative + diceTotal - 1) % WALL_SIDES;
  const keptStacks = Math.min(diceTotal, WALL_STACKS_PER_SIDE - 1);
  const stackAlongPath =
    ((WALL_SIDES - breakSide) % WALL_SIDES) * WALL_STACKS_PER_SIDE +
    keptStacks;
  return stackAlongPath * 2;
}

/**
 * Locates the upper tile of the physical dora stack.
 *
 * The rendered wall sequence ends on a lower tile, so the fifth and ninth
 * physical tiles from the end are represented by offsets six and ten.
 */
export function doraWallTileIndex(
  visibleTileCount: number,
  playerCount: number,
  indicatorIndex = 0,
  completedKanCount = 0,
): number {
  const initialOffset = playerCount === 3 ? 10 : 6;
  const direction = playerCount === 3 ? 1 : -1;
  return (
    visibleTileCount -
    initialOffset +
    indicatorIndex * 2 * direction +
    completedKanCount
  );
}

export function rinshanWallSlot(
  wallBreak: number,
  completedKanCount: number,
): number {
  return (
    wallBreak -
    Math.max(1, completedKanCount) +
    TOTAL_WALL_TILES
  ) % TOTAL_WALL_TILES;
}

export function tableLayoutZones() {
  const tileLength = TABLE_TILE_LENGTH * DEFAULT_TILE_SCALE;
  const tileWidth =
    TABLE_TILE_LENGTH * TILE_WIDTH_RATIO * DEFAULT_TILE_SCALE;
  const tileDepth =
    TABLE_TILE_LENGTH * TILE_DEPTH_RATIO * DEFAULT_TILE_SCALE;
  return {
    tile: {
      width: tileWidth,
      length: tileLength,
      depth: tileDepth,
    },
    river: {
      inner: 1.72 - tileLength / 2,
      outer:
        1.72 +
        2 * (tileLength + RIVER_TILE_GAP) +
        tileLength / 2,
    },
    wall: {
      inner: WALL_DISTANCE - tileLength / 2,
      outer: WALL_DISTANCE + tileLength / 2,
    },
    meld: {
      center: MELD_DISTANCE,
      inner: MELD_DISTANCE - tileLength / 2,
      outer: MELD_DISTANCE + tileLength / 2,
      rightEdge: MELD_RIGHT_EDGE,
    },
    hand: {
      center: HAND_DISTANCE,
      inner: HAND_DISTANCE - tileDepth / 2,
      outer: HAND_DISTANCE + tileDepth / 2,
    },
    table: {
      imageHalfSide: TABLE_IMAGE_SIDE / 2,
      clothHalfSide: TABLE_CLOTH_SIDE / 2,
    },
  };
}

/**
 * 纵向视野角固定，画面的屏幕缩放就正比于窗口高度——界面舞台也只按高度缩放，
 * 两边自然同步。窗口变宽只是左右各多露出一点桌面。
 */
const BASE_FOV = 20;

export function tableCameraLayout(aspect: number): {
  mode: "perspective" | "orthographic";
  fov: number;
  orthographicSize: number;
  y: number;
  z: number;
  targetY: number;
  targetZ: number;
} {
  /* 纵向视野角固定不随画幅变化，三维缩放才和界面舞台一致。 */
  void aspect;
  return {
    mode: "perspective",
    fov: BASE_FOV,
    orthographicSize: 13.4,
    y: 21,
    z: 18.2,
    targetY: 0.15,
    targetZ: 0.7,
  };
}

/**
 * 屏幕上一块矩形 → 三维里正好填满它的落点和缩放倍数。
 *
 * 主视角摸的那张牌是三维飞过来、二维接手的，两层各按各的尺寸摆就一定对不齐：
 * 三维那条手牌基线是照着别家的牌排的，比二维手牌窄一号也低一截，牌一落地就得
 * 跳一下大小和位置。所以这里反过来问镜头——屏幕上那一格，在眼前多深的地方摆
 * 一张多大的牌，投影出来正好把它填满。
 *
 * `depthReference` 只用来定深度：落点落在过它的那层等深面上，牌离镜头多远保持
 * 不变，只把横竖位置和大小对到那一格上。`unitWidth` 是缩放为 1 时牌面的世界
 * 宽度，返回的 `scale` 直接就是 group 该用的缩放。
 *
 * 镜头是可以在牌桌设置里调的，所以这一步只能问当前这台相机，写死一组数字换个
 * 视角就又错开了。
 */
export function screenRectAnchor(
  camera: THREE.Camera,
  canvas: { width: number; height: number },
  rect: { centerX: number; centerY: number; width: number },
  depthReference: THREE.Vector3,
  unitWidth: number,
): { position: THREE.Vector3; scale: number } {
  const depth = depthReference.clone().project(camera).z;
  const at = (x: number) =>
    new THREE.Vector3(
      (x / canvas.width) * 2 - 1,
      -(rect.centerY / canvas.height) * 2 + 1,
      depth,
    ).unproject(camera);
  const position = at(rect.centerX);
  const edge = at(rect.centerX + rect.width / 2);
  return {
    position,
    scale: (position.distanceTo(edge) * 2) / unitWidth,
  };
}

export function orthographicCameraBounds(
  size: number,
  aspect: number,
): { left: number; right: number; top: number; bottom: number } {
  const halfHeight = size / 2;
  return {
    left: -halfHeight * aspect,
    right: halfHeight * aspect,
    top: halfHeight,
    bottom: -halfHeight,
  };
}
