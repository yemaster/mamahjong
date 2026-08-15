import * as THREE from "three";
import { normalizeTileCode } from "../tileCounts";
import type { TableRuntime } from "./types";

/**
 * 拿起一张手牌时，桌上摊开的同种牌一起点亮。
 *
 * 找牌是玩家自己在看牌河和副露，眼睛得来回扫；把同一种牌压上一层浅蓝的淡影，
 * 一眼就能数清这张牌已经出了几枚。只登记翻开的牌——背面朝上的牌本来就看不出
 * 是什么，点亮它等于泄底。
 */

/** 淡影的颜色：往牌面上乘一层偏蓝的暗，像是被蓝光照着。 */
const HIGHLIGHT_TINT = new THREE.Color(0.63, 0.8, 1);

/**
 * 铳牌的颜色，只有牌谱重演会用到。
 *
 * 有人听这张牌，打出去就是放铳；红得比蓝影重一档，扫一眼牌河就知道哪些牌碰不得。
 */
const DANGER_TINT = new THREE.Color(1, 0.42, 0.42);

/** 记下一张摊在桌上的明牌，好在手牌被拿起时一起点亮。 */
export function registerTableTile(
  runtime: TableRuntime,
  group: THREE.Group,
  code: string,
): void {
  const plate = group.userData.facePlateMaterial as
    | THREE.MeshBasicMaterial
    | undefined;
  if (!plate) return;
  const key = normalizeTileCode(code);
  const existing = runtime.highlightMaterials.get(key);
  const entry = { material: plate, base: plate.color.clone() };
  group.userData.tableTileHighlight = { key, ...entry };
  if (existing) existing.push(entry);
  else runtime.highlightMaterials.set(key, [entry]);
}

/** 局部场景更新后，从仍在桌上的节点重建高亮索引。 */
export function rebuildTableTileHighlights(runtime: TableRuntime): void {
  runtime.highlightMaterials = new Map();
  runtime.root.traverse((object) => {
    const entry = object.userData.tableTileHighlight as
      | {
          key: string;
          material: THREE.MeshBasicMaterial;
          base: THREE.Color;
        }
      | undefined;
    if (!entry) return;
    /* 被新层顶替、等下一帧再释放的旧层已经不可见，不能重新放进索引。 */
    let ancestor: THREE.Object3D | null = object;
    while (ancestor) {
      if (!ancestor.visible) return;
      ancestor = ancestor.parent;
    }
    const existing = runtime.highlightMaterials.get(entry.key);
    const value = { material: entry.material, base: entry.base };
    if (existing) existing.push(value);
    else runtime.highlightMaterials.set(entry.key, [value]);
  });
}

/** 换一张要点亮的牌；`null` 表示手上没拿着牌，全部还原。 */
export function setTableTileHighlight(
  runtime: TableRuntime,
  code: string | null,
): void {
  const next = code == null ? null : normalizeTileCode(code);
  if (runtime.highlightedTileCode === next) return;
  runtime.highlightedTileCode = next;
  applyTableTileHighlight(runtime);
}

/**
 * 换一批铳牌；传空集合就是关掉提示。
 *
 * 只有牌谱重演会调它，对局里这个集合一直是空的。
 */
export function setTableDangerTiles(
  runtime: TableRuntime,
  codes: Iterable<string>,
): void {
  const next = new Set<string>();
  for (const code of codes) next.add(normalizeTileCode(code));
  if (
    next.size === runtime.dangerTileCodes.size &&
    [...next].every((code) => runtime.dangerTileCodes.has(code))
  ) {
    return;
  }
  runtime.dangerTileCodes = next;
  applyTableTileHighlight(runtime);
}

/** 把当前的点亮状态刷到材质上；局部层替换后也要再刷一遍。 */
export function applyTableTileHighlight(runtime: TableRuntime): void {
  for (const [key, entries] of runtime.highlightMaterials) {
    /* 拿起手牌那层蓝影压过铳牌的红：手上正拿着的牌是当下在找的东西。 */
    const tint =
      key === runtime.highlightedTileCode
        ? HIGHLIGHT_TINT
        : runtime.dangerTileCodes.has(key)
          ? DANGER_TINT
          : null;
    for (const entry of entries) {
      if (tint) entry.material.color.copy(entry.base).multiply(tint);
      else entry.material.color.copy(entry.base);
    }
  }
}
