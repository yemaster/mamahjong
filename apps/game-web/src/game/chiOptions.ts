import type { MatchView, ReactionOption, TileView } from "../types";

/** 一种吃牌方案：要从手上拿出去的那两张牌。 */
export interface ChiOption {
  /** 发指令要的两张手牌 id，顺序跟 `tiles` 一致。 */
  tileIds: [number, number];
  /** 这两张牌本身，已按牌序排好，画出来就是玩家看到的那一组。 */
  tiles: [TileView, TileView];
  /** 去重和 React key 用的牌码串，例如 `3s+0s`。 */
  key: string;
}

/**
 * 把服务端给的吃牌反应摊成一张张方案。
 *
 * 服务端本来就是一种方案一条 `{kind:"chi", tile_ids}`，这里只做三件事：把 id
 * 换回牌、把一组里的两张按牌序摆好、把牌码一样的方案并掉。
 *
 * 并掉重复是必要的：手上两张 `3s` 时，服务端按 id 枚举会给出两条「3s + 5s」，
 * 可玩家看到的是同一件事，列两遍只会让人以为两者有区别。红五是另一回事——`0s`
 * 和 `5s` 牌码不同，天然就是两条，不能一起并掉。
 */
export function chiOptions(
  reactions: ReactionOption[],
  hand: TileView[],
): ChiOption[] {
  const byId = new Map(hand.map((tile) => [tile.id, tile]));
  const seen = new Set<string>();
  const options: ChiOption[] = [];
  for (const reaction of reactions) {
    if (reaction.kind !== "chi") continue;
    const tiles = reaction.tile_ids.map((id) => byId.get(id));
    const [first, second] = tiles;
    /* 手上找不到这两张就画不出来，宁可少一条也不画半张牌。 */
    if (!first || !second) continue;
    const ordered: [TileView, TileView] =
      chiTileOrder(first.code) <= chiTileOrder(second.code)
        ? [first, second]
        : [second, first];
    const key = `${ordered[0].code}+${ordered[1].code}`;
    if (seen.has(key)) continue;
    seen.add(key);
    options.push({
      tileIds: [ordered[0].id, ordered[1].id],
      tiles: ordered,
      key,
    });
  }
  return options.sort(
    (left, right) =>
      chiTileOrder(left.tiles[0].code) - chiTileOrder(right.tiles[0].code) ||
      chiTileOrder(left.tiles[1].code) - chiTileOrder(right.tiles[1].code),
  );
}

/** 主视角这一手现在能吃的所有方案。 */
export function observerChiOptions(view: MatchView): ChiOption[] {
  const hand =
    view.players.find((player) => player.seat === view.observer_seat)
      ?.concealed_tiles ?? [];
  return chiOptions(view.available_reactions ?? [], hand);
}

/**
 * 吃只可能发生在数牌上，牌序按数字排就够了；红五（`0`）排在普通五后面，和手牌
 * 排序的口径一致。
 */
function chiTileOrder(code: string): number {
  const number = Number(code.slice(0, -1));
  return number === 0 ? 5.5 : number;
}
