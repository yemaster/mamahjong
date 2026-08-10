import * as THREE from "three";
import { TOTAL_WALL_TILES, WALL_STACKS_PER_SIDE } from "./constants";
import {
  impactWallTileOrigin,
  impactWallTileQuaternion,
  rinshanWallSlot,
  tableRelativeSeat,
  wallBreakSlot,
  wallTileOrigin,
  wallTileQuaternion,
} from "./geometry";

/**
 * 一副牌山怎么摆、按什么顺序摸。
 *
 * 两家麻将都是顺时针摸：每面墙内从右往左，走完一面接上家那面墙的右端。差别只在
 * 开门那面右端预留几墩、以及哪些牌不摸——立直留「点数之和」墩、山尾十四张是王牌；
 * 冲击麻将留「较小那颗点数」墩、只有翻财神那一墩不摸。
 * 差别全收在这个对象里，画牌山、发牌动画、摸牌动画都只跟它打交道。
 */
export interface WallLayout {
  /** 第 `order` 张被摸走的牌在牌山上的位置。`0` 就是开门那一张。 */
  drawSlot(order: number): number;
  /** 第 `completedKanCount` 个杠取到的岭上牌。 */
  rinshanSlot(completedKanCount: number): number;
  origin(slot: number, widthRatio: number, tileScale: number): THREE.Vector3;
  quaternion(slot: number): THREE.Quaternion;
  /** 摸牌序列有多长。 */
  drawableCount: number;
  /** 不参与摸牌、一直立在山上的位置。立直的王牌不算在内，它照旧走摸牌序列。 */
  deadSlots: number[];
  /** 开局就翻开的那一张；`null` 表示这一家的牌山上没有明牌。 */
  revealedSlot: number | null;
}

const WALL_SIDES = 4;
/** 每面墙的墩数。 */
const STACKS_PER_SIDE = WALL_STACKS_PER_SIDE;

export function riichiWallLayout(
  dealerRelative: number,
  dice: [number, number],
): WallLayout {
  const breakSlot = wallBreakSlot(dealerRelative, dice[0] + dice[1]);
  return {
    drawSlot: (order) => (breakSlot + order) % TOTAL_WALL_TILES,
    rinshanSlot: (completedKanCount) =>
      rinshanWallSlot(breakSlot, completedKanCount),
    origin: wallTileOrigin,
    quaternion: wallTileQuaternion,
    drawableCount: TOTAL_WALL_TILES,
    deadSlots: [],
    revealedSlot: null,
  };
}

/**
 * 冲击麻将的牌山布局。
 *
 * `dealer` 是庄家的**绝对**座次，`observer` 是本家座次；里面一律换成相对座次
 * （0=自家在屏幕下方，1=下家在右，2=对家在上，3=上家在左），槽位就是物理位置
 * `相对座次 * 34 + 墩号 * 2 + 层`，见 `impactWallTilePlacement`。
 *
 * 骰子 x=较小、y=较大。庄家算 1、逆时针数到 x+y 那一家是割目家；它面前那道墙
 * **右边**留 x 墩当牌山末尾，这 x 墩左侧就是牌山起点。
 *
 * 摸牌和立直一样是**顺时针**：每面墙内从右往左、先上层后下层，走完一面接**上家**
 * 那面墙的右端。所以整条路是
 * 割目家（左边 17-x 墩）→ 上家 → 对家 → 下家 → 割目家（右边预留的 x 墩）。
 *
 * 割目家的逆时针下家（庄家算 1 数到 x+y+1 的那家）面前那道墙，从左往右第 x+y 墩
 * 整墩不摸：上层翻开当财神指示牌，下层压在底下。
 */
export function impactWallLayout(
  dealer: number,
  observer: number,
  dice: [number, number],
): WallLayout {
  const diceSum = dice[0] + dice[1];
  const smaller = Math.min(dice[0], dice[1]);

  const dealerRelative = tableRelativeSeat(dealer, observer, WALL_SIDES);
  /* 割目家：庄家算 1，逆时针数到点数和。 */
  const breakSeat = (dealerRelative + diceSum - 1) % WALL_SIDES;

  /* 翻财神的是割目家的逆时针**下家**——庄家算 1 数到 x+y+1 的那家。 */
  const indicatorSeat = (breakSeat + 1) % WALL_SIDES;
  /* 它面前从左往右第 x+y 墩，墩号 0 起算就是 x+y-1；上层翻开。 */
  const indicatorStack = diceSum - 1;
  const indicatorSlot =
    indicatorSeat * STACKS_PER_SIDE * 2 + indicatorStack * 2;

  /* 割目家墙上右边预留 smaller 墩当牌山末尾。 */
  const reserved = smaller;

  const drawOrder: number[] = [];

  // 割目家：预留墩左边那 17-x 墩，从右往左，这是牌山开头。
  pushImpactStacks(drawOrder, breakSeat, 0, STACKS_PER_SIDE - reserved, indicatorSlot);

  // 顺时针接下去：上家(+3) → 对家(+2) → 下家(+1)。
  for (const step of [3, 2, 1]) {
    const wall = (breakSeat + step) % WALL_SIDES;
    pushImpactStacks(drawOrder, wall, 0, STACKS_PER_SIDE, indicatorSlot);
  }

  // 割目家：右边预留的那 x 墩，从右往左，这是牌山末尾。
  pushImpactStacks(
    drawOrder,
    breakSeat,
    STACKS_PER_SIDE - reserved,
    STACKS_PER_SIDE,
    indicatorSlot,
  );

  const drawableCount = drawOrder.length;
  return {
    drawSlot: (order) => drawOrder[order]!,
    /*
     * 杠张从摸牌序列的末尾取，但一墩之内照旧先上层后下层：第一个杠拿末尾那墩的
     * 上层（倒数第二张），第二个杠才拿压在底下的那张，第三、第四个杠退到上一墩。
     * 后端 `draw_from_back` 是同一套顺序。
     */
    rinshanSlot: (completedKanCount) =>
      drawOrder[rinshanOrderIndex(drawableCount, completedKanCount)]!,
    origin: impactWallTileOrigin,
    quaternion: impactWallTileQuaternion,
    drawableCount,
    deadSlots: [indicatorSlot, indicatorSlot + 1],
    revealedSlot: indicatorSlot,
  };
}

/**
 * 第 `completedKanCount` 个杠取到的那张，在摸牌序列里的下标。
 *
 * 从末尾一墩一墩往回退，每墩先上层（下标为偶、排在前面那张）后下层。
 */
export function rinshanOrderIndex(
  drawableCount: number,
  completedKanCount: number,
): number {
  const taken = Math.max(1, completedKanCount) - 1;
  const stacksBack = Math.floor(taken / 2) + 1;
  return drawableCount - stacksBack * 2 + (taken % 2);
}

/** 把一面墙的若干墩加入摸牌序列——从右往左。跳过财神指示牌那一墩。 */
function pushImpactStacks(
  order: number[],
  seat: number,
  start: number,
  end: number,
  indicatorSlot: number,
): void {
  const baseSlot = seat * STACKS_PER_SIDE * 2;
  for (let stack = end - 1; stack >= start; stack--) {
    const upperSlot = baseSlot + stack * 2;
    const lowerSlot = upperSlot + 1;
    if (upperSlot === indicatorSlot) continue;
    order.push(upperSlot);
    order.push(lowerSlot);
  }
}
