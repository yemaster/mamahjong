import type {
  GameCommandName,
  MatchView,
  TileView,
} from "../types";

/** 主视角回合可以选择的一组杠牌。 */
export interface KanOption {
  /** 暗杠需要牌号，立直还需要四张实际手牌 id；加杠则需要碰的副露和补上的那张。 */
  kind: "concealed" | "added";
  key: string;
  tiles: TileView[];
  tileIds?: [number, number, number, number];
  tileCode?: string;
  meldId?: number;
  tileId?: number;
}

/**
 * 把不同规则下的暗杠/加杠候选整理成同一组选择项。
 * 冲击麻将和四川麻将的服务端只给暗杠牌码，具体牌由服务端挑，所以这里仅用于
 * 画牌；立直麻将则保留四张手牌 id，点击后原样提交。
 */
export function observerKanOptions(view: MatchView): KanOption[] {
  const hand =
    view.players.find((player) => player.seat === view.observer_seat)
      ?.concealed_tiles ?? [];
  const byId = new Map(hand.map((tile) => [tile.id, tile]));
  const observer = view.players.find(
    (player) => player.seat === view.observer_seat,
  );
  const actions = view.turn_actions;
  const options: KanOption[] = [];

  if (view.variant_kind === "riichi") {
    for (const tileIds of actions.concealed_kan_tile_ids) {
      const tiles = tileIds.map((id) => byId.get(id));
      if (tiles.some((tile) => !tile)) continue;
      options.push({
        kind: "concealed",
        key: `concealed:${tileIds.join(",")}`,
        tiles: tiles as TileView[],
        tileIds,
      });
    }
  } else {
    const codes =
      view.variant_kind === "impact"
        ? actions.impact_concealed_kan_tile_codes ?? []
        : actions.sichuan_concealed_kan_tile_codes ?? [];
    for (const code of codes) {
      const tiles = hand.filter((tile) => tile.code === code);
      /* 服务端可能只下发牌码（例如冲击麻将财神指示牌只剩三张），仍然要能选。 */
      options.push({
        kind: "concealed",
        key: `concealed:${code}`,
        tiles: fillKanTiles(tiles, code, options.length),
        tileCode: code,
      });
    }
  }

  const added: Array<{ meldId: number; tileId?: number }> =
    view.variant_kind === "riichi"
      ? actions.added_kan_options.map((option) => ({
          meldId: option.meld_id,
          tileId: option.tile_id,
        }))
      : (view.variant_kind === "impact"
          ? actions.impact_added_kan_meld_ids ?? []
          : actions.sichuan_added_kan_meld_ids ?? []
        ).map((meldId) => ({ meldId }));

  for (const candidate of added) {
    const meld = observer?.melds.find((item) => item.id === candidate.meldId);
    if (!meld) continue;
    const tile =
      candidate.tileId != null
        ? byId.get(candidate.tileId)
        : hand.find((item) =>
            meld.tiles.some((meldTile) => meldTile.code === item.code),
          );
    /* 加杠牌码在服务端已经校验过；若当前快照还没带第四张，仍画出前三张，
       让选择框不会因为一帧不同步而消失。 */
    options.push({
      kind: "added",
      key: `added:${candidate.meldId}`,
      tiles: fillKanTiles(
        tile ? [...meld.tiles, tile] : meld.tiles,
        meld.tiles[0]?.code ?? "",
        options.length,
      ),
      meldId: candidate.meldId,
      tileId: candidate.tileId,
    });
  }

  return options.sort(
    (left, right) =>
      tileOrder(left.tiles[0]?.code) - tileOrder(right.tiles[0]?.code),
  );
}

/** 将选择项转换成对应规则命名空间的指令。 */
export function kanCommand(
  view: Pick<MatchView, "variant_kind">,
  option: KanOption,
): { name: GameCommandName; payload: unknown } {
  const prefix = view.variant_kind;
  if (option.kind === "added") {
    return {
      name: `${prefix}.added_kan` as GameCommandName,
      payload:
        view.variant_kind === "riichi"
          ? { meld_id: option.meldId, tile_id: option.tileId }
          : { meld_id: option.meldId },
    };
  }
  if (view.variant_kind === "riichi") {
    return {
      name: "riichi.concealed_kan",
      payload: { tile_ids: option.tileIds },
    };
  }
  return {
    name: `${prefix}.concealed_kan` as GameCommandName,
    payload: { tile_code: option.tileCode },
  };
}

function tileOrder(code = "") {
  const number = Number(code.slice(0, -1));
  const suit = code.slice(-1);
  const suitOrder =
    { m: 0, p: 10, s: 20, z: 30 }[suit as "m" | "p" | "s" | "z"] ?? 40;
  return suitOrder + (number === 0 ? 5.5 : number);
}

function fillKanTiles(
  tiles: TileView[],
  code: string,
  offset: number,
): TileView[] {
  const filled = [...tiles];
  while (filled.length < 4) {
    filled.push({ id: -(offset * 10 + filled.length + 1), code });
  }
  return filled;
}
