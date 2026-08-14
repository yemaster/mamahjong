import type {
  DiscardView,
  MatchPlayerView,
  MatchView,
  MeldView,
  TileView,
} from "../../types";
import { normalizeTileCode } from "../tileAssets";

/**
 * 万→筒→索→字，摸上来的那张单独留在末端。
 *
 * 冲击麻将传 `jokerCode`：财神是百搭，跟花色排不到一块儿，一律提到最左边。
 * 刚摸上来的那张仍然留在末端，哪怕它是财神——那一格的意思是「这张是新摸的」。
 */
export function sortHandForDisplay(
  tiles: TileView[],
  drawnTileId: number | null,
  jokerCode?: string | null,
): TileView[] {
  const drawn = tiles.find((tile) => tile.id === drawnTileId);
  const joker = jokerCode ? normalizeTileCode(jokerCode) : null;
  const concealed = tiles
    .filter((tile) => tile.id !== drawnTileId)
    .sort((left, right) => {
      const leftJoker = joker != null && normalizeTileCode(left.code) === joker;
      const rightJoker =
        joker != null && normalizeTileCode(right.code) === joker;
      if (leftJoker !== rightJoker) return leftJoker ? -1 : 1;
      return tileOrder(left.code) - tileOrder(right.code);
    });
  return drawn ? [...concealed, drawn] : concealed;
}

/** 这张牌是不是财神。立直麻将下 `jokerCode` 为空，恒为 `false`。 */
export function isJokerTile(
  code: string,
  jokerCode: string | null | undefined,
): boolean {
  if (!jokerCode) return false;
  return normalizeTileCode(code) === normalizeTileCode(jokerCode);
}

function tileOrder(code: string): number {
  const normalized = normalizeTileCode(code);
  const suitOrder: Record<string, number> = { m: 0, p: 1, s: 2, z: 3 };
  const suit = normalized.slice(-1);
  const rawNumber = Number(normalized.slice(0, -1));
  const number = rawNumber === 0 ? 5.5 : rawNumber;
  return (suitOrder[suit] ?? 4) * 20 + number;
}

export function canLocalPlayerDiscard(view: MatchView): boolean {
  return (
    (view.phase.kind === "awaiting_discard" ||
      view.phase.kind === "awaiting_turn_action") &&
    view.phase.seat === view.observer_seat
  );
}

export function playerIsHoldingDrawnTile(
  view: MatchView,
  seat: number,
): boolean {
  return (
    (view.phase.kind === "awaiting_discard" ||
      view.phase.kind === "awaiting_turn_action") &&
    view.phase.seat === seat
  );
}

/**
 * Index of the tile a 自摸 winner just drew, so it can be flipped up on its
 * own before the rest of the hand falls open.
 */
export function winningTileIndex(
  tiles: { id: number }[],
  drawnTileId: number | null,
): number {
  if (drawnTileId != null) {
    const index = tiles.findIndex((tile) => tile.id === drawnTileId);
    if (index >= 0) return index;
  }
  return tiles.length - 1;
}

/**
 * Seats that turn their hand face down once the winners have laid theirs out.
 *
 * 荣和 only exposes the player who dealt in; 自摸 and 流局 sweep every remaining
 * seat.
 */
export function settlementCoveringSeats(view: MatchView): number[] {
  const settlement = view.hand_settlement;
  if (!settlement) return [];
  const winnerSeats = settlement.winners.map((winner) => winner.seat);
  const others = view.players
    .map((player) => player.seat)
    .filter((seat) => !winnerSeats.includes(seat));
  if (settlement.reason === "ron") {
    return settlement.from_seat != null &&
      others.includes(settlement.from_seat)
      ? [settlement.from_seat]
      : [];
  }
  return others;
}

export interface RiverDiscardEntry {
  discard: DiscardView;
  originalIndex: number;
  /** 横置：立直宣言的那一张。 */
  sideways: boolean;
}

/**
 * 这张牌被鸣走了没有。
 *
 * 立直麻将记得到是被谁鸣走的（`claimed_by`），冲击麻将只记了有没有（`claimed`），
 * 两边都得认，不然冲击麻将副露完牌还赖在牌河里。
 */
function discardWasClaimed(discard: DiscardView): boolean {
  return discard.claimed_by != null || discard.claimed === true;
}

/**
 * 牌河上摆得出来的牌。被鸣走的牌不再占格子。
 *
 * 横置是牌河上唯一能看出谁立了直的记号，宣言牌被别人吃碰走之后这个记号就跟着
 * 牌一起离开了，得补回来：牌河里一张横置的都没有，就横下一张仍在牌河里的牌。
 */
export function riverDiscardEntries(
  discards: DiscardView[],
): RiverDiscardEntry[] {
  let sidewaysIndex = discards.findIndex((discard) => discard.riichi_declared);
  while (
    sidewaysIndex >= 0 &&
    sidewaysIndex < discards.length &&
    discardWasClaimed(discards[sidewaysIndex]!)
  ) {
    sidewaysIndex += 1;
  }
  return discards.flatMap((discard, originalIndex) =>
    discardWasClaimed(discard)
      ? []
      : [{ discard, originalIndex, sideways: originalIndex === sidewaysIndex }],
  );
}

export function countCompletedKans(view: MatchView): number {
  return view.players.reduce(
    (count, player) =>
      count +
      player.melds.filter((meld) =>
        ["open_kan", "concealed_kan", "added_kan"].includes(meld.kind),
      ).length,
    0,
  );
}

/**
 * 冲击麻将里真正已经摸走的岭上牌数。
 *
 * 副露会在杠点动画开始前就先变成杠；这时 `countCompletedKans` 已经加一，但岭上牌
 * 还留在牌山末尾。只有离开 `awaiting_kan_animation` 后，这一杠才算完成补摸。
 */
export function completedImpactRinshanDraws(view: MatchView): number {
  /* 新版接口直接给实际摸走数。它不会在杠点动画等待阶段抢跑，也不依赖客户端必须
     连续收到“杠成立”和“补牌完成”两帧；断线补帧或多份回执把版本挤在中间时，
     仍然能准确指出牌山末尾已经空了几个位置。 */
  if (view.completed_rinshan_draws != null) {
    return view.completed_rinshan_draws;
  }
  const kans = countCompletedKans(view);
  return view.variant_kind === "impact" &&
    view.phase.kind === "awaiting_kan_animation"
    ? Math.max(0, kans - 1)
    : kans;
}

/**
 * 本次摸牌若来自岭上，返回它在共享岭上序列中的编号（从 1 开始）。
 *
 * `pendingDrawNumber` 是正常动画等待帧留下的精确来源；计数跃迁是漏掉中间快照时
 * 的兜底。两者都没有时必须返回 `null`，否则普通摸牌会错误地从牌山末尾起飞。
 */
export function resolveRinshanDrawNumber(
  receivedDraw: boolean,
  pendingDrawNumber: number | undefined,
  completedDraws: number,
  previousCompletedDraws: number,
): number | null {
  if (!receivedDraw) return null;
  if (pendingDrawNumber != null) return pendingDrawNumber;
  return completedDraws > previousCompletedDraws ? completedDraws : null;
}

export function playerCompletedKan(
  player: MatchPlayerView,
  previousPlayer: MatchPlayerView | undefined,
): boolean {
  if (!previousPlayer) return false;
  const previousKinds = new Map(
    previousPlayer.melds.map((meld) => [meld.id, meld.kind]),
  );
  return player.melds.some(
    (meld) =>
      ["open_kan", "concealed_kan", "added_kan"].includes(meld.kind) &&
      previousKinds.get(meld.id) !== meld.kind,
  );
}

/** 当前视图是否比上一帧多了一张拔北牌。 */
export function playerExtractedNorth(
  player: MatchPlayerView,
  previousPlayer: MatchPlayerView | undefined,
): boolean {
  return (
    previousPlayer != null &&
    (player.nuki_tiles?.length ?? 0) >
      (previousPlayer.nuki_tiles?.length ?? 0)
  );
}

/** 当前视图是否首次出现了这家的新摸入牌。 */
export function playerReceivedDraw(
  view: MatchView,
  previousView: MatchView | null,
  player: MatchPlayerView,
  previousPlayer: MatchPlayerView | undefined,
): boolean {
  if (!previousView || !previousPlayer) return false;
  if (!playerIsHoldingDrawnTile(view, player.seat)) return false;

  /*
   * 冲击麻将的杠会隔着一帧杠点动画。补摸这一帧最可靠的信号不是副露再次变化，
   * 而是「上一帧正在等这家播杠动画，这一帧牌山少了一张并轮到这家」。加杠尤其
   * 需要这条：碰升级为杠的变化早在响应/动画帧里已经消费掉了。
   */
  const receivedAfterImpactKan =
    view.variant_kind === "impact" &&
    previousView.variant_kind === "impact" &&
    previousView.phase.kind === "awaiting_kan_animation" &&
    previousView.phase.seat === player.seat &&
    view.remaining_live_draws < previousView.remaining_live_draws;

  const tracksRinshanDraws =
    view.completed_rinshan_draws != null &&
    previousView.completed_rinshan_draws != null;
  const receivedRinshanDraw =
    tracksRinshanDraws &&
    view.completed_rinshan_draws! > previousView.completed_rinshan_draws!;

  if (player.seat === view.observer_seat) {
    return (
      player.drawn_tile_id != null &&
      (player.drawn_tile_id !== previousPlayer.drawn_tile_id ||
        receivedAfterImpactKan ||
        receivedRinshanDraw)
    );
  }

  return (
    !playerIsHoldingDrawnTile(previousView, player.seat) ||
    player.concealed_tile_count > previousPlayer.concealed_tile_count ||
    receivedAfterImpactKan ||
    receivedRinshanDraw ||
    (!tracksRinshanDraws &&
      (playerCompletedKan(player, previousPlayer) ||
        playerExtractedNorth(player, previousPlayer)))
  );
}

export interface MeldDisplayTile {
  tile: TileView;
  calledRotation: number;
  addedBesideCalled: boolean;
  /** 暗杠两端的那两张牌是扣着的。 */
  faceDown: boolean;
}

/**
 * 副露摆放顺序。返回的数组是从**右往左**排的，第 0 张在最右边。
 *
 * 被鸣的那张横过来，横在谁的方向上就说明是从谁那儿鸣的：下家的放最右、对家的
 * 放中间、上家的放最左。暗杠没人放铳，四张全部直立，两端两张扣着盖住牌面。
 * 加杠的第四张叠在被鸣的那张旁边。
 */
export function meldDisplayTiles(
  meld: MeldView,
  sourceRelative: number | null,
): MeldDisplayTile[] {
  const called = meld.tiles.find((tile) => tile.id === meld.called_tile_id);
  if (!called || sourceRelative == null) {
    const concealedKan = meld.kind === "concealed_kan";
    return meld.tiles.map((tile, index) => ({
      tile,
      calledRotation: 0,
      addedBesideCalled: false,
      faceDown:
        concealedKan && (index === 0 || index === meld.tiles.length - 1),
    }));
  }
  const allOthers = meld.tiles.filter((tile) => tile.id !== called.id);
  const addedTile =
    meld.kind === "added_kan" ? allOthers.at(-1) : undefined;
  const others = addedTile
    ? allOthers.filter((tile) => tile.id !== addedTile.id)
    : allOthers;
  const calledEntry: MeldDisplayTile = {
    tile: called,
    calledRotation: sourceRelative === 3 ? -Math.PI / 2 : Math.PI / 2,
    addedBesideCalled: false,
    faceDown: false,
  };
  let base: MeldDisplayTile[];
  if (sourceRelative === 3) {
    /* 上家：横置牌在最左，剩下的倒过来排，从左往右读还是顺子的顺序。 */
    base = [...others.slice().reverse().map(noRotation), calledEntry];
  } else if (sourceRelative === 2) {
    /* 对家：横置牌落在从左数第二张；碰是三张，第二张正好就是中间那张。 */
    base = [
      ...others.slice(0, others.length - 1).map(noRotation),
      calledEntry,
      ...others.slice(-1).map(noRotation),
    ];
  } else {
    /* 下家：横置牌在最右，也就是数组的第 0 张。 */
    base = [calledEntry, ...others.map(noRotation)];
  }
  if (!addedTile) return base;
  return base.flatMap((entry) =>
    entry.tile.id === called.id
      ? [
          entry,
          {
            tile: addedTile,
            calledRotation: calledEntry.calledRotation,
            addedBesideCalled: true,
            faceDown: false,
          },
        ]
      : [entry],
  );
}

function noRotation(tile: TileView): MeldDisplayTile {
  return {
    tile,
    calledRotation: 0,
    addedBesideCalled: false,
    faceDown: false,
  };
}
