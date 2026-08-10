import type { MatchView, MeldView, TileView } from "../types";
import { normalizeTileCode } from "../game/tileCounts";

/**
 * 牌谱重演用的听牌求解器。
 *
 * 正式对局的听牌一律由服务端的 `WinQuery` 判定，这份实现只服务牌谱：拖进度条时
 * 每一步都要重算四家，来回请求服务端撑不住。两份实现只在牌谱这条路上并存，
 * 这里的结果不参与任何正式对局的判定（见 `docs/match-record-replay.md` 第六节）。
 *
 * 牌按 34 种计数：万 0-8、筒 9-17、索 18-26、字 27-33。赤五和普通五算同一种。
 */

/** 34 种牌的计数数组。 */
export type TileCounts = Int8Array;

const SUIT_OFFSET: Record<string, number> = { m: 0, p: 9, s: 18, z: 27 };

/** 幺九牌（国士要用）：三色的 1/9 加七种字牌。 */
const TERMINALS_AND_HONORS = new Set([
  0, 8, 9, 17, 18, 26, 27, 28, 29, 30, 31, 32, 33,
]);

/** 取一格计数。开了 `noUncheckedIndexedAccess`，下标读出来先兜一次底。 */
function at(counts: TileCounts, index: number): number {
  return counts[index] ?? 0;
}

/** 牌码转 0-33 的序号；认不出来的牌返回 `-1`。 */
export function tileIndex(code: string): number {
  const normalized = normalizeTileCode(code);
  const rank = Number(normalized[0]);
  const offset = SUIT_OFFSET[normalized[1] ?? ""];
  if (!Number.isInteger(rank) || offset === undefined) return -1;
  if (rank < 1 || rank > (offset === 27 ? 7 : 9)) return -1;
  return offset + rank - 1;
}

/** 0-33 的序号转回牌码，赤五一律写成普通五。 */
export function tileCodeAt(index: number): string {
  if (index >= 27) return `${index - 27 + 1}z`;
  const suit = index >= 18 ? "s" : index >= 9 ? "p" : "m";
  return `${(index % 9) + 1}${suit}`;
}

/** 一把牌数成 34 种的计数。 */
export function countTiles(codes: Iterable<string>): TileCounts {
  const counts = new Int8Array(34);
  for (const code of codes) {
    const index = tileIndex(code);
    if (index >= 0) counts[index] = at(counts, index) + 1;
  }
  return counts;
}

/**
 * 标准型：`setsNeeded` 个面子，雀头已经在外面摘掉了。
 *
 * 从最小的牌号往上啃，同一张牌先试刻子再试顺子——最小的那张牌无论如何都得被
 * 某个面子吃掉，只有这两种吃法，所以贪心地从头拆不会漏解。
 */
function decompose(counts: TileCounts, setsNeeded: number): boolean {
  let index = 0;
  while (index < 34 && at(counts, index) === 0) index += 1;
  if (index === 34) return setsNeeded === 0;
  if (setsNeeded === 0) return false;

  if (at(counts, index) >= 3) {
    counts[index] = at(counts, index) - 3;
    const ok = decompose(counts, setsNeeded - 1);
    counts[index] = at(counts, index) + 3;
    if (ok) return true;
  }
  /* 顺子只有数牌有，而且不能跨花色，所以只到每花色的第七张为止。 */
  if (
    index < 27 &&
    index % 9 <= 6 &&
    at(counts, index + 1) > 0 &&
    at(counts, index + 2) > 0
  ) {
    counts[index] = at(counts, index) - 1;
    counts[index + 1] = at(counts, index + 1) - 1;
    counts[index + 2] = at(counts, index + 2) - 1;
    const ok = decompose(counts, setsNeeded - 1);
    counts[index] = at(counts, index) + 1;
    counts[index + 1] = at(counts, index + 1) + 1;
    counts[index + 2] = at(counts, index + 2) + 1;
    if (ok) return true;
  }
  return false;
}

/** 标准型和了判定：先定雀头，剩下的全拆成面子。 */
function isStandardWin(counts: TileCounts, setsNeeded: number): boolean {
  if (setsNeeded < 0) return false;
  for (let index = 0; index < 34; index += 1) {
    if (at(counts, index) < 2) continue;
    counts[index] = at(counts, index) - 2;
    const ok = decompose(counts, setsNeeded);
    counts[index] = at(counts, index) + 2;
    if (ok) return true;
  }
  return false;
}

/** 七对子：十四张、七种、每种正好两张。副露过就不成立。 */
function isSevenPairs(counts: TileCounts, meldCount: number): boolean {
  if (meldCount > 0) return false;
  let pairs = 0;
  for (let index = 0; index < 34; index += 1) {
    const count = at(counts, index);
    if (count === 0) continue;
    if (count !== 2) return false;
    pairs += 1;
  }
  return pairs === 7;
}

/** 国士无双：十三种幺九各至少一张，其中一种两张。副露过就不成立。 */
function isThirteenOrphans(counts: TileCounts, meldCount: number): boolean {
  if (meldCount > 0) return false;
  let total = 0;
  let paired = false;
  for (let index = 0; index < 34; index += 1) {
    const count = at(counts, index);
    if (count === 0) continue;
    if (!TERMINALS_AND_HONORS.has(index) || count > 2) return false;
    if (count === 2) {
      if (paired) return false;
      paired = true;
    }
    total += count;
  }
  return paired && total === 14;
}

/** 十四张的和了判定，三种型都算。 */
export function isWinningHand(counts: TileCounts, meldCount: number): boolean {
  return (
    isStandardWin(counts, 4 - meldCount) ||
    isSevenPairs(counts, meldCount) ||
    isThirteenOrphans(counts, meldCount)
  );
}

/**
 * 一把 13 张形（副露折算在 `meldCount` 里）听什么。
 *
 * 逐个试 34 种牌，摸进来能和的就是听的牌。这里不看桌上还剩几枚——听牌就是听牌，
 * 哪怕四张都在别人手上；剩余枚数由 {@link waitRemainingCount} 单独算。
 */
export function waitsFromCounts(
  counts: TileCounts,
  meldCount: number,
): string[] {
  const waits: string[] = [];
  for (let index = 0; index < 34; index += 1) {
    /* 自己手上已经有四张的牌摸不到第五张，那不叫听。 */
    if (at(counts, index) >= 4) continue;
    counts[index] = at(counts, index) + 1;
    const winning = isWinningHand(counts, meldCount);
    counts[index] = at(counts, index) - 1;
    if (winning) waits.push(tileCodeAt(index));
  }
  return waits;
}

/**
 * 某一家现在听什么。
 *
 * 刚摸完牌的时候手上是 14 张，那一刻严格来说还没定型；重演要一直显示听牌状态，
 * 就先把刚摸的那张摘掉，按「摸切之后听什么」算。
 */
export function seatWaits(
  concealed: TileView[],
  melds: MeldView[],
  drawnTileId: number | null = null,
): string[] {
  let tiles = concealed;
  if (tiles.length % 3 === 2) {
    const index = tiles.findIndex((tile) => tile.id === drawnTileId);
    tiles =
      index >= 0
        ? [...tiles.slice(0, index), ...tiles.slice(index + 1)]
        : tiles.slice(0, -1);
  }
  if (tiles.length % 3 !== 1) return [];
  return waitsFromCounts(
    countTiles(tiles.map((tile) => tile.code)),
    melds.length,
  );
}

/**
 * 牌桌上看得见的牌的枚数。
 *
 * 和对局中的 `visibleTileCounts` 不是一回事：重演里四家的手牌全是明的，所以四家
 * 手牌、所有副露、所有牌河都数进去。宝牌指示牌不算——剩余枚数的口径就是
 * 「手牌 + 副露 + 牌河」这三样。被鸣走的弃牌已经进了副露，牌河里那一份要跳过，
 * 否则同一张数两遍。
 */
export function replayVisibleCounts(view: MatchView): Map<string, number> {
  const counts = new Map<string, number>();
  const add = (code: string) => {
    const key = normalizeTileCode(code);
    counts.set(key, (counts.get(key) ?? 0) + 1);
  };
  for (const player of view.players) {
    for (const tile of player.concealed_tiles ?? []) add(tile.code);
    for (const meld of player.melds) {
      for (const tile of meld.tiles) add(tile.code);
    }
    for (const discard of player.discards) {
      if (discard.claimed_by == null) add(discard.tile.code);
    }
  }
  return counts;
}

/** 听的这几种牌一共还剩几枚：一种四张，减掉桌上看得见的。 */
export function waitRemainingCount(
  waits: string[],
  visible: Map<string, number>,
): number {
  return waits.reduce(
    (total, code) =>
      total + Math.max(0, 4 - (visible.get(normalizeTileCode(code)) ?? 0)),
    0,
  );
}

/** 某一家的听牌角标要显示的两样。 */
export interface SeatWaitInfo {
  waits: string[];
  remaining: number;
}

/** 四家各自听什么、还剩几枚；没听牌的座位不进这张表。 */
export function seatWaitInfo(view: MatchView): Map<number, SeatWaitInfo> {
  const visible = replayVisibleCounts(view);
  const table = new Map<number, SeatWaitInfo>();
  for (const player of view.players) {
    const concealed = player.concealed_tiles;
    if (!concealed || concealed.length === 0) continue;
    const waits = seatWaits(concealed, player.melds, player.drawn_tile_id);
    if (waits.length === 0) continue;
    table.set(player.seat, {
      waits,
      remaining: waitRemainingCount(waits, visible),
    });
  }
  return table;
}
