import type { EndReason, MatchResult, TileView } from "../types";

/**
 * 牌谱 JSON 的 TS 侧类型，对应 `crates/mamahjong-application/src/record.rs`。
 *
 * 归档目录里躺着的旧牌谱缺几个后来才加的字段（开局时间、好友标记、牌山），
 * 所以这里能缺的一律写成可选或可空——重演要降级，不能直接崩。
 * 详见 `docs/match-record-replay.md`。
 */

/* ── 列表 ───────────────────────────────────────────────── */

export interface RecordSeatSummary {
  seat: number;
  nickname: string;
  rank: number;
  /** 素点：终局那一刻手上的点数。 */
  points: number;
  /**
   * 算上返点和马点之后的最终得分，单位是十分之一。
   *
   * 列表上的「增减」写的就是这个数，不是素点减起始点数——输赢由马点定。
   */
  score_tenths: number;
}

export interface MatchRecordSummary {
  match_id: string;
  /** 归档文件的修改时间，也就是这局打完的时刻。 */
  finished_at_ms: number;
  /** 旧牌谱认不出好友还是匹配，标题就只写规则部分。 */
  friend_match: boolean | null;
  /** 麻将种类：规则集 ID 斜杠前那一截（`riichi`）。旧牌谱没有，标题少这一段。 */
  rule_family: string | null;
  variant: "yonma" | "sanma" | null;
  match_length: "east_only" | "hanchan" | null;
  /**
   * 规则名：预设写预设短名（「ML规则」「A规」），动过预设或自己捏的写「自定义规则」。
   *
   * 服务端读牌谱的时候拿快照跟预设逐字段比出来的，不存在归档里——预设改版之后，
   * 存死的名字就成了旧账。
   */
  rule_name: string | null;
  hand_count: number;
  seats: RecordSeatSummary[];
}

export interface MatchRecordListResponse {
  schema: string;
  records: MatchRecordSummary[];
}

/* ── 牌谱本体 ───────────────────────────────────────────── */

export interface RecordPlayer {
  user_id: string;
  seat: number;
  nickname: string;
}

/**
 * 一局洗好的牌山。
 *
 * `tiles[0..live_end]` 是活牌区、按摸牌先后排，之后固定十四张王牌。
 * 对局没结束时服务端一律下发 `null`——把牌山发给还在打的客户端等于发一份作弊器。
 */
export interface WallSnapshot {
  tiles: TileView[];
  live_end: number;
}

/**
 * 一家和牌的番符明细。
 *
 * 形状和实时对局的 `WinnerSettlementView`（`types.ts`）一致——重演的结算面板就是
 * 对局那一块 `HandSettlement`，喂进去的必须是同一种东西。
 */
export interface RecordWinnerScore {
  seat: number;
  han: number;
  fu: number;
  yakuman_multiplier: number;
  limit: string;
  points: number;
  dealer: boolean;
  yaku: Array<{ name: string; value: number; yakuman: boolean }>;
}

export interface RecordEvent {
  sequence: number;
  name: string;
  event_version: number;
  payload: Record<string, unknown>;
}

export interface HandRecord {
  hand_index: number;
  round_wind: "east" | "south" | "west" | "north";
  round_number: number;
  dealer: number;
  honba: number;
  riichi_sticks: number;
  reason: EndReason;
  points_before: number[];
  point_deltas: number[];
  points_after: number[];
  winners: number[];
  /**
   * 和牌那几家的番符与役种，和 `winners` 同序。
   *
   * 后来才加的字段：旧牌谱没有，那几局就不放结算面板（重演照样能走完）。
   */
  winner_scores?: RecordWinnerScore[];
  /** 本局的里宝牌指示牌；流局不翻，旧牌谱也没有。 */
  ura_dora_indicators?: TileView[];
  from: number | null;
  tenpai: number[];
  nagashi_winners: number[];
  awarded_riichi_sticks: number;
  dealer_continues: boolean;
  first_event_sequence: number | null;
  last_event_sequence: number | null;
  /** 旧牌谱没有牌山，牌山面板要降级成一行说明。 */
  wall?: WallSnapshot | null;
  events: RecordEvent[];
}

export interface MatchRecord {
  schema: string;
  match_id: string;
  version: number;
  event_sequence: number;
  /** 旧牌谱没有这个字段，标题就只写规则部分。 */
  friend_match?: boolean;
  rule_snapshot: RuleSnapshot;
  /** 服务端读的时候现算的规则名，和列表里那一列是同一个函数出来的。 */
  rule_name?: string;
  players: RecordPlayer[];
  hands: HandRecord[];
  result: MatchResult | null;
}

export interface RuleSnapshot {
  /** 「麻将种类/人数」，例如 `riichi/yonma`。 */
  rule_set_id?: string;
  config?: {
    variant?: "yonma" | "sanma";
    match_rules?: {
      length?: "east_only" | "hanchan";
      initial_points?: number;
    };
  };
}

/* ── 事件负载 ───────────────────────────────────────────── */

export interface HandStartedPayload {
  round_wind: string;
  round_number: number;
  dealer: number;
  honba: number;
  riichi_sticks: number;
  dora_indicator: TileView;
  remaining_live_draws: number;
}

export interface InitialHandDealtPayload {
  seat: number;
  tiles: TileView[];
}

export interface TileDrawnPayload {
  seat: number;
  tile: TileView;
  source: "live_wall" | "rinshan";
  remaining_live_draws: number;
}

export interface TileDiscardedPayload {
  seat: number;
  tile: TileView;
  /**
   * 摸切还是手切。
   *
   * 早年的牌谱没写这一条，缺了就当手切——宁可不压暗，也不能凭空给一张牌安上摸切。
   */
  tsumogiri?: boolean;
  riichi_declared: boolean;
}

export interface MeldPayload {
  seat: number;
  meld: {
    id: number;
    kind: "chi" | "pon" | "open_kan" | "concealed_kan" | "added_kan";
    tiles: TileView[];
    called_from: number | null;
    called_tile_id: number | null;
  };
}

export interface DoraIndicatorRevealedPayload {
  tile: TileView;
  revealed_count: number;
}

export interface RiichiEstablishedPayload {
  seat: number;
  points_after: number[];
  riichi_sticks: number;
}
