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
