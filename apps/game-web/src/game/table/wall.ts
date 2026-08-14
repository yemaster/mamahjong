import type { TileView } from "../../types";
import { openingDealOrder, openingDealStep, standUpEase } from "./animation";
import { DORA_FLIP_MS } from "./dice";
import {
  OPENING_DEAL_STEP_MS,
  WALL_TILE_LENGTH,
} from "./constants";
import { makeTile, rootTile, tileBody } from "./tileMesh";
import type { TableRuntime } from "./types";
import type { WallLayout } from "./wallLayout";

/**
 * 往牌山上摆一张牌。给了 `flipAt` 就先扣着牌背，到点再当场翻过来。
 *
 * 翻的是真牌体：绕自身长轴转半圈，`π` 那头朝上的是牌背，转到 `0` 才露出正面。
 * 不是换贴图，所以中途看得见牌立起来的那道边。
 */
export interface WallTileSpec {
  slot: number;
  code: string | null;
  flipAt: number | null;
}

/** 计算每张起手牌轮到从物理牌山起飞的绝对时刻，包含主视角座位。 */
export function openingWallTakeoffSchedule(
  layout: WallLayout,
  players: Array<{ seat: number; concealed_tile_count: number }>,
  dealer: number,
  seatCount: number,
  startedAt: number,
): Map<number, number> {
  const schedule = new Map<number, number>();
  for (const player of players) {
    for (let index = 0; index < player.concealed_tile_count; index += 1) {
      const order = openingDealOrder(index, player.seat, dealer, seatCount);
      const step = openingDealStep(index, player.seat, dealer, seatCount);
      schedule.set(
        layout.drawSlot(order),
        startedAt + step * OPENING_DEAL_STEP_MS,
      );
    }
  }
  return schedule;
}

export function addWallTile(
  runtime: TableRuntime,
  layout: WallLayout,
  slot: number,
  code: string | null,
  flipAt: number | null,
): void {
  const group = makeTile(runtime, code ?? "back", WALL_TILE_LENGTH);
  const position = layout.origin(
    slot,
    runtime.tileWidthRatio,
    runtime.tileScale,
  );
  group.position.copy(position);
  group.quaternion.copy(layout.quaternion(slot));
  if (code != null && flipAt != null) {
    const body = tileBody(group);
    body.rotation.x = Math.PI;
    runtime.tilts.push({
      object: body,
      group,
      startX: Math.PI,
      endX: 0,
      startPosition: position.clone(),
      endPosition: position.clone(),
      startedAt: flipAt,
      duration: DORA_FLIP_MS,
      covering: false,
      ease: standUpEase,
    });
  }
  rootTile(runtime, group);
}

/**
 * 立直的牌墙：从骰子决定的开口处往后铺，翻开的宝牌指示牌露出正面，其余都是背面。
 *
 * `doraFlipAt` 是开局那一次翻宝牌的时刻；传 `null` 表示这张牌本来就该是亮的
 * （杠宝牌、刷新后重建的牌山），直接正面出场。
 */
export function addWall(
  runtime: TableRuntime,
  layout: WallLayout,
  remainingLiveDraws: number,
  doraIndicators: TileView[],
  completedRinshanDraws: number,
  showEntireWall: boolean,
  doraFlipAt: number | null = null,
): void {
  for (const tile of riichiWallTiles(
    layout,
    remainingLiveDraws,
    doraIndicators,
    completedRinshanDraws,
    showEntireWall,
    doraFlipAt,
  )) {
    addWallTile(runtime, layout, tile.slot, tile.code, tile.flipAt);
  }
}

/** 返回当前仍在桌上的立直牌山槽位，供增量场景按槽复用。 */
export function riichiWallTiles(
  layout: WallLayout,
  remainingLiveDraws: number,
  doraIndicators: TileView[],
  completedRinshanDraws: number,
  showEntireWall: boolean,
  doraFlipAt: number | null = null,
): WallTileSpec[] {
  const liveEnd = layout.drawableCount - 14;
  const consumedTileCount = showEntireWall
    ? 0
    : liveEnd - remainingLiveDraws - completedRinshanDraws;
  const removedRinshan = new Set(
    Array.from({ length: showEntireWall ? 0 : completedRinshanDraws }, (_, index) =>
      layout.rinshanOrderIndex(index + 1),
    ),
  );
  const doraByIndex = new Map(
    doraIndicators.map((tile, indicatorIndex) => [
      layout.doraOrderIndex(indicatorIndex),
      tile.code,
    ]),
  );
  const tiles: WallTileSpec[] = [];
  for (let order = consumedTileCount; order < layout.drawableCount; order += 1) {
    if (removedRinshan.has(order)) continue;
    const doraCode = doraByIndex.get(order);
    tiles.push({
      slot: layout.drawSlot(order),
      code: doraCode ?? null,
      flipAt: doraCode != null ? doraFlipAt : null,
    });
  }
  return tiles;
}

/**
 * 冲击麻将的牌墙。
 *
 * 这一家没有王牌区，山上唯一的明牌是财神指示牌——翻财神那一墩不参与摸牌，
 * 从头到尾立在原处，所以它不跟着摸牌序列走，单独摆一次。
 */
export function addImpactWall(
  runtime: TableRuntime,
  layout: WallLayout,
  remainingDraws: number,
  completedKanCount: number,
  jokerIndicator: TileView | undefined,
  indicatorFlipAt: number | null = null,
): void {
  for (const tile of impactWallTiles(
    layout,
    remainingDraws,
    completedKanCount,
    jokerIndicator,
    indicatorFlipAt,
  )) {
    addWallTile(runtime, layout, tile.slot, tile.code, tile.flipAt);
  }
}

/** 返回当前仍在桌上的冲击麻将牌山槽位，供增量场景按槽复用。 */
export function impactWallTiles(
  layout: WallLayout,
  remainingDraws: number,
  completedKanCount: number,
  jokerIndicator: TileView | undefined,
  indicatorFlipAt: number | null = null,
): WallTileSpec[] {
  const consumedTileCount =
    layout.drawableCount - remainingDraws - completedKanCount;
  /*
   * 杠张是从末尾一墩一墩往回取的，一墩之内先上层后下层，所以摸掉的不一定是连着
   * 的一段：只杠过一次，末尾那墩的上层没了、下层还立着。挨个问 `rinshanSlot`
   * 才知道少了哪几张。
   */
  const takenByKan = new Set<number>();
  for (let kan = 1; kan <= completedKanCount; kan += 1) {
    takenByKan.add(layout.rinshanSlot(kan));
  }
  const tiles: WallTileSpec[] = [];
  for (let index = consumedTileCount; index < layout.drawableCount; index += 1) {
    const slot = layout.drawSlot(index);
    if (takenByKan.has(slot)) continue;
    tiles.push({ slot, code: null, flipAt: null });
  }
  for (const slot of layout.deadSlots) {
    const revealed = slot === layout.revealedSlot;
    tiles.push({
      slot,
      code: revealed ? (jokerIndicator?.code ?? null) : null,
      flipAt: revealed ? indicatorFlipAt : null,
    });
  }
  return tiles;
}
