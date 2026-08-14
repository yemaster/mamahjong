import type { TileView } from "../../types";
import { standUpEase } from "./animation";
import { DORA_FLIP_MS } from "./dice";
import { WALL_TILE_LENGTH } from "./constants";
import { makeTile, rootTile, tileBody } from "./tileMesh";
import type { TableRuntime } from "./types";
import type { WallLayout } from "./wallLayout";

/**
 * 往牌山上摆一张牌。给了 `flipAt` 就先扣着牌背，到点再当场翻过来。
 *
 * 翻的是真牌体：绕自身长轴转半圈，`π` 那头朝上的是牌背，转到 `0` 才露出正面。
 * 不是换贴图，所以中途看得见牌立起来的那道边。
 */
function addWallTile(
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
  for (let order = consumedTileCount; order < layout.drawableCount; order += 1) {
    if (removedRinshan.has(order)) continue;
    const doraCode = doraByIndex.get(order);
    addWallTile(
      runtime,
      layout,
      layout.drawSlot(order),
      doraCode ?? null,
      doraCode != null ? doraFlipAt : null,
    );
  }
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
  for (let index = consumedTileCount; index < layout.drawableCount; index += 1) {
    const slot = layout.drawSlot(index);
    if (takenByKan.has(slot)) continue;
    addWallTile(runtime, layout, slot, null, null);
  }
  for (const slot of layout.deadSlots) {
    const revealed = slot === layout.revealedSlot;
    addWallTile(
      runtime,
      layout,
      slot,
      revealed ? (jokerIndicator?.code ?? null) : null,
      revealed ? indicatorFlipAt : null,
    );
  }
}
