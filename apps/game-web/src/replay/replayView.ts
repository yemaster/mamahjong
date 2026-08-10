import type {
  HandSettlementView,
  MatchPhase,
  MatchPlayerView,
  MatchView,
} from "../types";
import type { MatchRecord } from "./recordTypes";
import type { ReplayHandState } from "./replayState";

/**
 * 把折叠出来的牌桌状态拼成一份合成的 `MatchView`。
 *
 * 重演不另写一套牌桌：正式对局那套 `GameTable` / `MatchHud` / `PlayerHand2D` 认的
 * 就是 `MatchView`，喂一份手工拼的进去就能画出一模一样的桌子（`TableSettingsScene`
 * 已经这么干了）。
 */

/** 牌桌的重绘只认 `view.id`；整场重演固定一个 id，换步不重建场景。 */
export function replayViewId(matchId: string): string {
  return `replay:${matchId}`;
}

/**
 * 当前该谁动，折成牌桌认的相位。
 *
 * 主视角自己那一手一律写成 `awaiting_responses`：`awaiting_discard` 会把 2D 手牌
 * 变成可点的，而重演里点牌不该打出任何东西。别人那一手照写 `awaiting_discard`，
 * 牌桌靠它给刚摸的那张牌留空、头像也跟着亮。
 */
function replayPhase(
  activeSeat: number | null,
  observerSeat: number,
): MatchPhase {
  if (activeSeat != null && activeSeat !== observerSeat) {
    return { kind: "awaiting_discard", seat: activeSeat };
  }
  return { kind: "awaiting_responses", trigger_seat: activeSeat ?? observerSeat };
}

export interface ReplayViewInput {
  record: MatchRecord;
  state: ReplayHandState;
  /** `MatchRecord.hands` 的下标。 */
  handIndex: number;
  observerSeat: number;
  /** 每走一步都要换个数：牌桌的重绘 effect 盯的就是它。 */
  version: number;
  /** 这一局开局时四家的点数。 */
  pointsBefore: number[];
  /**
   * 走到本局最后一步、而且这一局是有人和牌收的，才给结算。
   *
   * 一给上，牌桌就进结算相位：`settlementRevealSeats` 里的手牌摊平、和牌那张翻起来，
   * 结算面板跟着升起。流局一律不给（见 `replaySettlement.ts`）。
   */
  settlement?: HandSettlementView | null;
}

export function buildReplayView({
  record,
  state,
  handIndex,
  observerSeat,
  version,
  pointsBefore,
  settlement = null,
}: ReplayViewInput): MatchView {
  const players: MatchPlayerView[] = state.seats.map((seat) => {
    const identity = record.players.find((player) => player.seat === seat.seat);
    return {
      user_id: identity?.user_id ?? `replay-seat-${seat.seat}`,
      seat: seat.seat,
      nickname: identity?.nickname ?? `座位${seat.seat + 1}`,
      avatar_path: null,
      /* 牌谱只记座位和牌，不记当时各家用的什么角色，所以回放里不喊语音。 */
      character_id: null,
      character_illustration_path: null,
      points: pointsBefore[seat.seat] ?? 0,
      concealed_tiles: seat.concealed,
      concealed_tile_count: seat.concealed.length,
      drawn_tile_id: seat.drawnTileId,
      melds: seat.melds,
      discards: seat.discards,
      riichi_status: seat.riichi,
      /*
       * 听牌提示走的是 HUD 上的角标，不是手牌上方那块面板：那块面板是对局中
       * 给自己看的，还要 `has_yaku`，牌谱这边算不出来，索性不给。
       */
      waiting_tiles: [],
      furiten: false,
    };
  });

  return {
    schema: "match.v1",
    variant_kind: "riichi",
    id: replayViewId(record.match_id),
    room_id: "",
    version,
    event_sequence: version,
    hand_index: handIndex,
    observer_seat: observerSeat,
    progress: state.progress,
    phase: replayPhase(state.activeSeat, observerSeat),
    remaining_live_draws: state.remainingLiveDraws,
    dora_indicators: state.doraIndicators,
    players,
    available_reactions: [],
    turn_actions: {
      can_tsumo: false,
      riichi_discard_tile_ids: [],
      riichi_discard_hints: [],
      tenpai_discard_hints: [],
      concealed_kan_tile_ids: [],
      added_kan_options: [],
      can_nine_terminals: false,
    },
    clocks: [],
    hand_settlement: settlement,
    result: null,
    friend_match: record.friend_match ?? false,
    can_start_exit_vote: false,
    exit_vote: null,
    terminated_by_exit_vote: false,
  };
}

/** 这一局开局时四家的点数；旧牌谱缺这段就退回起始点数。 */
export function handPointsBefore(
  record: MatchRecord,
  handIndex: number,
): number[] {
  const hand = record.hands[handIndex];
  if (hand?.points_before?.length) return hand.points_before;
  const initial =
    record.rule_snapshot?.config?.match_rules?.initial_points ?? 25000;
  return record.players.map(() => initial);
}
