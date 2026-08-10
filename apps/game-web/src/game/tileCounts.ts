import type { MatchView } from "../types";

/** 赤宝牌和同数字的普通牌是同一种牌，统计前先归一成 `5m/5p/5s`。 */
export function normalizeTileCode(code: string): string {
  return /^0[mps]$/.test(code) ? `5${code[1]}` : code;
}

/**
 * 当前视角已经能看见的各种牌的枚数。
 *
 * 能看见的是：自己的暗手、四家牌河、四家副露，以及翻开的宝牌指示牌。被鸣走的
 * 弃牌已经算进副露，牌河里那一份要跳过，否则同一张牌数两遍。
 */
export function visibleTileCounts(view: MatchView): Map<string, number> {
  const counts = new Map<string, number>();
  const add = (code: string) => {
    const key = normalizeTileCode(code);
    counts.set(key, (counts.get(key) ?? 0) + 1);
  };

  for (const indicator of view.dora_indicators ?? []) add(indicator.code);
  for (const player of view.players) {
    if (player.seat === view.observer_seat) {
      for (const tile of player.concealed_tiles ?? []) add(tile.code);
    }
    for (const discard of player.discards) {
      if (discard.claimed_by == null) add(discard.tile.code);
    }
    for (const meld of player.melds) {
      for (const tile of meld.tiles) add(tile.code);
    }
  }
  return counts;
}

/** 某种牌还剩几枚没露面：一种四张，减掉已经看见的。 */
export function tileRemaining(
  visible: Map<string, number>,
  code: string,
): number {
  return Math.max(0, 4 - (visible.get(normalizeTileCode(code)) ?? 0));
}
