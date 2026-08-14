import type {
  DiscardView,
  EndReason,
  MeldView,
  ProgressView,
  TileView,
} from "../types";
import type {
  DoraIndicatorRevealedPayload,
  HandRecord,
  HandStartedPayload,
  InitialHandDealtPayload,
  MeldPayload,
  NorthExtractedPayload,
  RiichiEstablishedPayload,
  TileDiscardedPayload,
  TileDrawnPayload,
} from "./recordTypes";

/**
 * 把一局的事件日志折叠成某一步的牌桌状态。
 *
 * 牌谱里每一家的牌都是明的，所以任何一步都能从局首重放出来，不需要服务端再算
 * 一遍，也不需要维护增量：每走一步就从头折一次，一局最多两百来个事件，折一遍
 * 比维护一套可撤销的增量便宜得多，也不会有累积误差。
 */

export interface ReplaySeatState {
  seat: number;
  concealed: TileView[];
  melds: MeldView[];
  nukiTiles: TileView[];
  discards: DiscardView[];
  riichi: "none" | "pending" | "established";
  /** 刚摸上来还没打的那张。 */
  drawnTileId: number | null;
}

export interface ReplayHandState {
  progress: ProgressView;
  doraIndicators: TileView[];
  remainingLiveDraws: number;
  seats: ReplaySeatState[];
  /** 当前轮到谁；打完牌等着别人应答的时候是 `null`。 */
  activeSeat: number | null;
  /** 每一张已经被摸走的牌是谁摸的，牌山面板靠它分黄/灰/白。 */
  drawnBy: Map<number, number>;
  ended: boolean;
  endReason: EndReason | null;
}

function emptySeat(seat: number): ReplaySeatState {
  return {
    seat,
    concealed: [],
    melds: [],
    nukiTiles: [],
    discards: [],
    riichi: "none",
    drawnTileId: null,
  };
}

/** 从暗手里拿掉指定 id 的那张；不在手上就当没这回事（加杠会碰到）。 */
function removeTile(tiles: TileView[], tileId: number): void {
  const index = tiles.findIndex((tile) => tile.id === tileId);
  if (index >= 0) tiles.splice(index, 1);
}

/**
 * 折叠到 `throughEventIndex`（含）为止的牌桌状态。
 *
 * `throughEventIndex` 传 -1 就是一局开始前的空桌。
 */
export function foldHand(
  hand: HandRecord,
  seatCount: number,
  throughEventIndex: number,
): ReplayHandState {
  const state: ReplayHandState = {
    progress: {
      round_wind: hand.round_wind,
      round_number: hand.round_number,
      dealer: hand.dealer,
      honba: hand.honba,
      riichi_sticks: hand.riichi_sticks,
    },
    doraIndicators: [],
    remainingLiveDraws: 0,
    seats: Array.from({ length: seatCount }, (_, seat) => emptySeat(seat)),
    activeSeat: null,
    drawnBy: new Map(),
    ended: false,
    endReason: null,
  };

  const events = hand.events ?? [];
  const last = Math.min(throughEventIndex, events.length - 1);
  for (let index = 0; index <= last; index += 1) {
    const event = events[index];
    if (!event) continue;
    const payload = event.payload as unknown;

    switch (event.name) {
      case "riichi.hand_started": {
        const started = payload as HandStartedPayload;
        state.progress = {
          round_wind: started.round_wind,
          round_number: started.round_number,
          dealer: started.dealer,
          honba: started.honba,
          riichi_sticks: started.riichi_sticks,
        };
        state.doraIndicators = [started.dora_indicator];
        state.remainingLiveDraws = started.remaining_live_draws;
        break;
      }

      case "riichi.initial_hand_dealt": {
        const dealt = payload as InitialHandDealtPayload;
        const seat = state.seats[dealt.seat];
        if (!seat) break;
        seat.concealed = [...dealt.tiles];
        for (const tile of dealt.tiles) state.drawnBy.set(tile.id, dealt.seat);
        break;
      }

      case "riichi.tile_drawn": {
        const drawn = payload as TileDrawnPayload;
        const seat = state.seats[drawn.seat];
        if (!seat) break;
        seat.concealed.push(drawn.tile);
        seat.drawnTileId = drawn.tile.id;
        state.remainingLiveDraws = drawn.remaining_live_draws;
        state.activeSeat = drawn.seat;
        state.drawnBy.set(drawn.tile.id, drawn.seat);
        break;
      }

      case "riichi.tile_discarded": {
        const discarded = payload as TileDiscardedPayload;
        const seat = state.seats[discarded.seat];
        if (!seat) break;
        removeTile(seat.concealed, discarded.tile.id);
        seat.discards.push({
          tile: discarded.tile,
          /* 旧牌谱没写摸切，缺了一律当手切：不压暗好过压错。 */
          tsumogiri: discarded.tsumogiri ?? false,
          riichi_declared: discarded.riichi_declared,
          claimed_by: null,
        });
        seat.drawnTileId = null;
        /* 立直棒是下一条 `riichi_established` 才落桌的，这里先记宣言。 */
        if (discarded.riichi_declared && seat.riichi === "none") {
          seat.riichi = "pending";
        }
        state.activeSeat = null;
        break;
      }

      case "riichi.north_extracted": {
        const extracted = payload as NorthExtractedPayload;
        const seat = state.seats[extracted.seat];
        if (!seat) break;
        removeTile(seat.concealed, extracted.tile.id);
        seat.nukiTiles.push(extracted.tile);
        if (seat.drawnTileId === extracted.tile.id) seat.drawnTileId = null;
        break;
      }

      case "riichi.meld_declared":
      case "riichi.kan_completed": {
        const declared = payload as MeldPayload;
        const seat = state.seats[declared.seat];
        const meld = declared.meld;
        if (!seat || !meld) break;
        const fromOther =
          meld.called_from != null && meld.called_from !== declared.seat;
        const placedDrawnTile = meld.tiles.some(
          (tile) =>
            tile.id === seat.drawnTileId &&
            !(fromOther && tile.id === meld.called_tile_id),
        );
        for (const tile of meld.tiles) {
          /* 被鸣的那张来自别人的牌河，本来就不在手上。 */
          if (fromOther && tile.id === meld.called_tile_id) continue;
          removeTile(seat.concealed, tile.id);
        }
        /* 加杠沿用碰的那一组，按 id 覆盖而不是再挂一组。 */
        const existing = seat.melds.findIndex((entry) => entry.id === meld.id);
        if (existing >= 0) seat.melds[existing] = meld;
        else seat.melds.push(meld);
        if (fromOther && meld.called_tile_id != null) {
          const source = state.seats[meld.called_from!];
          const claimed = source?.discards.find(
            (discard) => discard.tile.id === meld.called_tile_id,
          );
          if (claimed) claimed.claimed_by = declared.seat;
        }
        if (
          event.name === "riichi.kan_completed" ||
          fromOther ||
          placedDrawnTile
        ) {
          seat.drawnTileId = null;
        }
        state.activeSeat = declared.seat;
        break;
      }

      case "riichi.dora_indicator_revealed": {
        const revealed = payload as DoraIndicatorRevealedPayload;
        state.doraIndicators = [
          ...state.doraIndicators.slice(0, revealed.revealed_count - 1),
          revealed.tile,
        ];
        break;
      }

      case "riichi.riichi_established": {
        const established = payload as RiichiEstablishedPayload;
        const seat = state.seats[established.seat];
        if (seat) seat.riichi = "established";
        state.progress = {
          ...state.progress,
          riichi_sticks: established.riichi_sticks,
        };
        break;
      }

      case "riichi.tsumo_declared":
        state.ended = true;
        state.endReason = "tsumo";
        break;

      case "riichi.ron_declared":
        state.ended = true;
        state.endReason = "ron";
        break;

      case "riichi.abortive_draw_declared":
      case "riichi.exhaustive_draw_declared": {
        const reason = (payload as { reason?: string }).reason;
        state.ended = true;
        state.endReason = (reason as EndReason | undefined) ?? "exhaustive_draw";
        break;
      }

      default:
        break;
    }
  }

  return state;
}
