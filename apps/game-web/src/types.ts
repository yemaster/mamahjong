/** Schema versions carried in every API response. */
export interface ApiEnvelope {
  schema: string;
}

export interface ApiFailure {
  code: string;
  message: string;
}

/* ── Auth ──────────────────────────────────────────────────── */

export interface AuthResponse extends ApiEnvelope {
  user: UserView;
  session: SessionView;
}

export interface UserView {
  id: string;
  version: number;
  login_name: string;
  status: string;
  role: string;
  profile: ProfileView;
}

export interface ProfileView {
  nickname: string;
  equipped_title: TitleView | null;
  selected_character: CharacterView | null;
  selected_outfit_id: string | null;
  avatar_path: string | null;
  selected_tablecloth_id: string | null;
  selected_lobby_music_id: string | null;
  selected_match_music_id: string | null;
  selected_riichi_music_id: string | null;
  ranks: RankView[];
}

export interface TitleView {
  id: string;
  name: string;
}

export interface CharacterView {
  id: string;
  name: string;
}

export interface CharacterAsset {
  name: string;
  path: string;
}

/**
 * 一条语音对应牌桌上的哪个动作。
 *
 * 挑语音只认它，不认 `name`——名字是管理端能随手改的展示文案。旧数据里没有这
 * 个字段，读出来就是 `undefined`，那种语音只在试听里出现。
 */
export type VoiceKind =
  | "riichi"
  | "double_riichi"
  | "chi"
  | "pon"
  | "kan"
  | "nuki"
  | "ron"
  | "tsumo";

export interface CharacterVoice {
  kind?: VoiceKind | null;
  name: string;
  path: string;
}

export interface CharacterOutfit {
  id: string;
  name: string;
  illustration_path: string;
}

export interface LobbyCharacter {
  id: string;
  version: number;
  name: string;
  illustration_path: string;
  emotes: CharacterAsset[];
  voices: CharacterVoice[];
  outfits: CharacterOutfit[];
  enabled: boolean;
  is_default: boolean;
}

export interface DefaultCharacterResponse extends ApiEnvelope {
  character: LobbyCharacter;
}

export interface CharacterListResponse extends ApiEnvelope {
  characters: LobbyCharacter[];
}

export interface TableclothView {
  id: string;
  version: number;
  name: string;
  texture_path: string;
  enabled: boolean;
  is_default: boolean;
}

export interface TableclothListResponse extends ApiEnvelope {
  tablecloths: TableclothView[];
}

export type MusicScene = "lobby" | "match" | "riichi";

export interface MusicTrackView {
  id: string;
  version: number;
  name: string;
  scene: MusicScene;
  audio_path: string;
  duration_ms: number;
  enabled: boolean;
  is_default: boolean;
}

export interface MusicTrackListResponse extends ApiEnvelope {
  music_tracks: MusicTrackView[];
}

export interface PlayerStatistics {
  matches_played: number;
  first_places: number;
  second_places: number;
  third_places: number;
  fourth_places: number;
  hands_played: number;
  wins: number;
  ron_wins: number;
  tsumo_wins: number;
  deal_ins: number;
  riichi_count: number;
  highest_hand_gain: number;
  average_rank: number;
  recent_matches: RecentMatch[];
}

export interface RecentMatch {
  match_id: string;
  rank: number;
  final_points: number;
  hands: number;
  wins: number;
}

export interface UserProfileDetail extends ApiEnvelope {
  user: UserView;
  statistics: PlayerStatistics;
}

export interface RankView {
  rule_set_id: string;
  queue_id: string;
  rank: string;
  points: number;
}

export interface SessionView {
  id: string;
  token: string;
  token_type: string;
}

export interface UserActivity extends ApiEnvelope {
  kind: "idle" | "room" | "game" | "matchmaking";
  room_id: string | null;
  match_id: string | null;
  ticket_id: string | null;
}

/* ── Rules ─────────────────────────────────────────────────── */

export interface RuleSetSummary {
  id: string;
  name: string;
  seat_count: number;
}

export interface RuleSetCatalog extends ApiEnvelope {
  rule_sets: RuleSetDetail[];
}

/** 规则家族。设置面板与牌桌渲染都按它分支，配置字段两边完全不同。 */
export type MahjongFamily = "riichi" | "impact";

export interface RuleSetDetail {
  id: string;
  family: MahjongFamily;
  display_name: string;
  seat_count: number;
  presets: RulePreset[];
  default_config: RuleConfig;
}

export interface RulePreset {
  id: string;
  revision: number;
  display_name: string;
  config: RuleConfig;
}

/** `family` 是判别字段：读之前先看它，别拿立直的字段去认冲击的配置。 */
export type RuleConfig = RiichiRuleConfig | ImpactRuleConfig;

export type RiichiVariant = "yonma" | "sanma";
export type MatchLength = "east_only" | "hanchan";
export type DealerContinuation = "win_only" | "win_or_tenpai";
export type SanmaNorthRule = "nuki_dora" | "yakuhai";
export type YakumanValue = "stacked_only" | "double_variants_and_stacked";
export type KuikaeRule = "forbidden" | "same_tile_only" | "allowed";
export type RonResolution = "head_bump" | "multiple";

export type PlacementUma =
  | { type: "fixed"; values: number[] }
  | { type: "jpml_a" };

export interface RiichiRuleConfig {
  variant: RiichiVariant;
  match_rules: {
    length: MatchLength;
    initial_points: number;
    return_points: number;
    first_place_required_points: number;
    thinking_time: {
      base_seconds: number;
      reserve_seconds: number;
    };
    tobi: boolean;
    dealer_continuation: DealerContinuation;
    agari_yame: boolean;
    north?: SanmaNorthRule;
  };
  scoring: {
    kiriage_mangan: boolean;
    old_yaku: boolean;
    yakuman_value: YakumanValue;
    nagashi_mangan: boolean;
    kazoe_yakuman: boolean;
    kokushi_ankan_chankan: boolean;
  };
  calls: {
    kuitan: boolean;
    kuikae: KuikaeRule;
  };
  bonuses: {
    red_fives: {
      man: number;
      pin: number;
      sou: number;
    };
    ippatsu: boolean;
    ura_dora: boolean;
    kan_dora: boolean;
  };
  abortive_draws: {
    four_winds: boolean;
    four_kans: boolean;
    nine_terminals: boolean;
    four_riichi: boolean;
  };
  settlement: {
    uma: PlacementUma;
    noten_payment: number;
    ron_resolution: RonResolution;
  };
}

export type ImpactMode = "blind" | "bright";

/**
 * 冲击麻将的规则配置。
 *
 * 对局设置只有思考秒数——长度由「有人点数归零」决定，起始点数（100）、
 * 杠点起始（0）、连庄条件都是规则写死的，没有可调项。
 */
export interface ImpactRuleConfig {
  mode: ImpactMode;
  match_rules: {
    thinking_time: {
      base_seconds: number;
      reserve_seconds: number;
    };
  };
  kan: ImpactKanRules;
  special: {
    /** 七嵌：手牌可分为 7 组，每组是同花色相差恰好 2 的两张数牌。默认关。 */
    seven_gaps: boolean;
  };
  all_in: ImpactAllInRules;
}

export interface ImpactKanRules {
  /** 开启则加杠只有被碰那家付 3 杠点，关闭则其余三家各付 1。 */
  added_kan_single_payer: boolean;
  /** 碰/暗杠财神指示牌按杠结算杠点，但牌型仍算刻子。 */
  indicator_pon_counts_as_kan: boolean;
  /** 庄家首打后三家依次打出同一种牌，庄家向其余三家各付 1 杠点。 */
  first_round_repeat_discard: boolean;
  /** 打出四张相同牌算杠：牌河中四张相同牌向三人各收 1 杠点。 */
  four_identical_discards_as_kan: boolean;
  /** 手牌 ≦4 张时碰牌收杠点：明碰收 3 点，明杠收 6 点。 */
  pon_with_few_tiles_as_kan: boolean;
}

/** 开启即胡出该牌型直接全交（胜者 400、其余归零），关闭则改为额外 +10 点。 */
export interface ImpactAllInRules {
  eleven_honor_streak: boolean;
  all_honors: boolean;
  pure_flush_no_joker: boolean;
  single_wait: boolean;
  three_kans: boolean;
  four_jokers: boolean;
  pure_seven_pairs: boolean;
  last_tile: boolean;
  blessing: boolean;
}

/* ── Rooms ─────────────────────────────────────────────────── */

export interface RoomList extends ApiEnvelope {
  rooms: RoomView[];
}

/**
 * 房间带的规则快照。
 *
 * 服务端发的是整份配置，但房间页只用得上顶栏那一行需要的这几项，所以这里只声明这几
 * 项——两家的字段互不相干，各自可选，读之前先看房间的 `variant_kind`。要完整配置的
 * 强类型（建房页那种）用 `RuleConfig`。
 */
export interface RoomRuleSnapshotView {
  rule_set_id: string;
  config: {
    /** 立直：三麻还是四麻。 */
    variant?: RiichiVariant;
    /** 立直：东风战还是半庄。 */
    match_rules?: { length?: MatchLength };
    /** 冲击麻将：模式。 */
    mode?: ImpactMode;
  };
}

export interface RoomView extends ApiEnvelope {
  id: string;
  version: number;
  owner_user_id: string;
  name: string;
  visibility: "public" | "private";
  lifecycle: "waiting" | "playing" | "closed";
  seat_count: number;
  /** 规则家族。房间页照它挑设置面板与开局后的牌桌渲染分支。 */
  variant_kind: MahjongFamily;
  /** 规则名：预设短名（「A规」「ML规则」）、「标准规则」或「自定义规则」，服务端算好的。 */
  rule_name: string;
  rule_snapshot: RoomRuleSnapshotView;
  members: RoomMemberView[];
  active_match_id: string | null;
}

export interface RoomMemberView {
  user_id: string;
  seat: number;
  nickname: string;
  ready: boolean;
  character: {
    id: string;
    name: string;
    illustration_path: string;
  };
}

export interface StartMatchResponse extends ApiEnvelope {
  match_id: string;
  room: RoomView;
}

/* ── Matchmaking ───────────────────────────────────────────── */

export interface MatchmakingTicket extends ApiEnvelope {
  id: string;
  rule_set_id: string;
  status: "waiting" | "matched" | "cancelled";
  match_id: string | null;
}

/* ── Matches ───────────────────────────────────────────────── */

/**
 * 一张牌桌。两套规则共用这一个形状。
 *
 * 只属于冲击麻将的字段一律可选，立直那条路径上读到的就是 `undefined`；
 * 读它们之前先看 `variant_kind`。
 */
export interface MatchView extends ApiEnvelope {
  id: string;
  room_id: string;
  version: number;
  event_sequence: number;
  hand_index: number;
  observer_seat: number;
  variant_kind: MahjongFamily;
  progress: ProgressView;
  phase: MatchPhase;
  remaining_live_draws: number;
  /** 已实际摸走的岭上牌数量；拔北或杠的响应窗口中尚不增加。 */
  completed_rinshan_draws?: number;
  dora_indicators: TileView[];
  /** 三麻的北规则；四麻不下发。 */
  sanma_north_rule?: SanmaNorthRule;
  /** 冲击麻将：唯一那张财神指示牌，画在左上角。 */
  joker_indicator?: TileView;
  /** 由指示牌推出来的财神牌码；手牌里凡是这张都当百搭。 */
  joker_code?: string;
  /** 连庄次数。中央罗盘与左上角都写「连庄 X 次」。 */
  dealer_streak?: number;
  /** 本局用的整套冲击麻将规则，按钮文案与帮助页照它回显。 */
  impact_rules?: ImpactRuleConfig;
  /** 最近一次杠点变动，播完即弃的浮层照它渲染。 */
  last_kan?: KanPointsView;
  players: MatchPlayerView[];
  available_reactions: ReactionOption[];
  turn_actions: TurnActions;
  clocks: SeatClockView[];
  opening_ready_seats?: number[];
  assets_ready_seats?: number[];
  terminated_by_asset_timeout?: boolean;
  hand_settlement: HandSettlementView | null;
  result: MatchResult | null;
  friend_match: boolean;
  can_start_exit_vote: boolean;
  exit_vote: ExitVoteView | null;
  terminated_by_exit_vote: boolean;
}

/** 一次杠（或第一巡连打）带来的杠点增减。 */
export interface KanPointsView {
  /** 单调递增；客户端靠它认出「这是新的一次」而不是同一次的重发。 */
  id: number;
  /** 引发这次变动的座位。谁付谁收看 `deltas`，第一巡连打里两者不是同一人。 */
  seat: number;
  kind: KanPointsKind;
  deltas: number[];
}

export type KanPointsKind =
  | "open_kan"
  | "concealed_kan"
  | "added_kan"
  | "indicator_pon"
  | "indicator_concealed"
  | "first_round_repeat_discard"
  | "four_identical_discards"
  | "four_consecutive_discards"
  | "three_indicator_discards"
  | "three_consecutive_indicator"
  | "chankan";

export interface ExitVoteView {
  initiator_seat: number;
  remaining_ms: number;
  votes: Array<boolean | null>;
}

export interface HandSettlementView {
  reason: EndReason;
  tenpai_seats: number[];
  point_deltas: number[];
  points_before: number[];
  points_after: number[];
  winners: WinnerSettlementView[];
  /** 已经报告结算动画播完的座位。 */
  played_seats: number[];
  /** 确认窗口剩下的时间；`null` 表示窗口还没开，确认按钮不该出现。 */
  confirm_remaining_ms: number | null;
  confirmed_seats: number[];
  from_seat: number | null;
  ura_dora_indicators: TileView[];
  /** 触发的全交牌型名。有值时役种表只写这一条，合计写「全交」。 */
  all_in?: string;
  kan_point_deltas?: number[];
  kan_points_after?: number[];
  /** 荒牌：本局不算，同一个庄家直接重开。 */
  void_hand?: boolean;
}

export interface WinnerSettlementView {
  seat: number;
  han: number;
  fu: number;
  yakuman_multiplier: number;
  limit: string;
  points: number;
  dealer: boolean;
  yaku: {
    name: string;
    value: number;
    yakuman: boolean;
  }[];
}

export interface ProgressView {
  round_wind: string;
  round_number: number;
  dealer: number;
  honba: number;
  riichi_sticks: number;
}

export type MatchPhase =
  | { kind: "awaiting_turn_action"; seat: number }
  | { kind: "awaiting_discard"; seat: number }
  | { kind: "awaiting_responses"; trigger_seat: number }
  | { kind: "awaiting_kan_animation"; seat: number }
  | { kind: "ended"; reason: EndReason };

export type EndReason =
  | "exhaustive_draw"
  | "nine_terminals"
  | "four_winds"
  | "four_kans"
  | "four_riichi"
  | "tsumo"
  | "ron";

export interface MatchPlayerView {
  user_id: string;
  seat: number;
  nickname: string;
  avatar_path: string | null;
  /** 这一家用的角色，照它挑操作语音。 */
  character_id: string | null;
  character_illustration_path: string | null;
  points: number;
  concealed_tiles: TileView[] | null;
  concealed_tile_count: number;
  drawn_tile_id: number | null;
  melds: MeldView[];
  /** 三麻已经拔出的北牌。 */
  nuki_tiles: TileView[];
  discards: DiscardView[];
  riichi_status: "none" | "pending" | "established";
  waiting_tiles: WaitingTileView[];
  furiten: boolean;
  /** 冲击麻将的杠点，另记一本账，可以为负。点数后面括号里写的就是它。 */
  kan_points?: number;
  /** 本局已经杠了几次（指示牌碰/暗杠不计），满 3 触发三杠全交。 */
  kan_count?: number;
  /** 自己连续打出字牌或财神的次数，满 11 触发连打十一风全交。 */
  honor_streak?: number;
  /** 立直音乐文件路径；没选就是空。 */
  riichi_music_path?: string | null;
}

export interface TileView {
  id: number;
  code: string;
}

export interface MeldView {
  id: number;
  kind: MeldKind;
  tiles: TileView[];
  called_from: number | null;
  called_tile_id: number | null;
}

/**
 * `indicator_pon` / `indicator_concealed` 是冲击麻将独有的：杠点按明杠/暗杠结算，
 * 但牌型仍然是刻子——不摸岭上牌，也不算杠上开花。
 */
export type MeldKind =
  | "chi"
  | "pon"
  | "open_kan"
  | "concealed_kan"
  | "added_kan"
  | "indicator_pon"
  | "indicator_concealed";

export interface DiscardView {
  tile: TileView;
  tsumogiri: boolean;
  riichi_declared: boolean;
  claimed_by: number | null;
  /** 这张被鸣走了。冲击麻将只记了有没有，记不到是谁。 */
  claimed?: boolean;
}

export type ReactionOption =
  | { kind: "ron" }
  | { kind: "chi"; tile_ids: [number, number] }
  | { kind: "pon"; tile_ids: [number, number] }
  | { kind: "open_kan"; tile_ids: [number, number, number] }
  /** 冲击麻将的碰：凑数的两张由服务端挑，所以不带牌号。 */
  | { kind: "impact_pon"; indicator: boolean }
  | { kind: "impact_open_kan" };

export interface TurnActions {
  can_tsumo: boolean;
  riichi_discard_tile_ids: number[];
  riichi_discard_hints: DiscardWaitHint[];
  /** 打哪张能听、听什么；立不立直都算。 */
  tenpai_discard_hints: DiscardWaitHint[];
  concealed_kan_tile_ids: [number, number, number, number][];
  added_kan_options: AddedKanOption[];
  nuki_tile_ids: number[];
  can_nine_terminals: boolean;
  /** 冲击麻将的暗杠：报牌码而不是牌号，具体哪四张由服务端挑。 */
  impact_concealed_kan_tile_codes?: string[];
  impact_added_kan_meld_ids?: number[];
  /** 手里三张财神指示牌，可以按暗杠结算杠点（牌型仍是刻子）。 */
  impact_indicator_concealed_kan?: boolean;
}

export interface DiscardWaitHint {
  tile_id: number;
  waiting_tiles: WaitingTileView[];
}

export interface WaitingTileView {
  code: string;
  has_yaku: boolean;
}

export interface AddedKanOption {
  meld_id: number;
  tile_id: number;
}

export interface SeatClockView {
  seat: number;
  remaining_ms: number;
  base_ms: number;
  reserve_ms: number;
}

export interface MatchResult {
  /** 冲击麻将走 `points_exhausted`：有人点数归零，整场就收。 */
  end_reason: string;
  hand_count: number;
  final_points: number[];
  placements: Placement[];
  unclaimed_riichi_sticks_awarded: number;
  /** 冲击麻将整场的杠点结余，结算页要和点数增减一起列。 */
  kan_points?: number[];
  point_deltas?: number[];
}

export interface Placement {
  seat: number;
  rank: number;
  points: number;
  uma_tenths: number;
  oka_tenths: number;
  score_tenths: number;
}

/* ── Commands ──────────────────────────────────────────────── */

export type GameCommandName =
  | "riichi.discard"
  | "riichi.riichi_discard"
  | "riichi.tsumo"
  | "riichi.ron"
  | "riichi.pass"
  | "riichi.nine_terminals"
  | "riichi.chi"
  | "riichi.pon"
  | "riichi.open_kan"
  | "riichi.concealed_kan"
  | "riichi.added_kan"
  | "riichi.nuki"
  | "game.ready_for_hand"
  | "game.settlement_played"
  | "game.confirm_settlement"
  | "impact.discard"
  | "impact.tsumo"
  | "impact.ron"
  | "impact.chi"
  | "impact.pon"
  | "impact.open_kan"
  | "impact.concealed_kan"
  | "impact.added_kan"
  | "impact.indicator_concealed_kan"
  | "impact.pass"
  | "impact.kan_animation_played"
  | "game.request_exit_vote"
  | "game.vote_exit"
  | "game.assets_ready";

/* ── WebSocket ─────────────────────────────────────────────── */

export interface WsTicketResponse {
  schema: "ws_ticket.v1";
  ticket: string;
  expires_in: number;
}

export interface WsWelcome {
  kind: "welcome";
  schema: "welcome.v1";
  connection_id: string;
  protocol: string;
  heartbeat_interval: number;
  streams: WsStreamState[];
}

export interface WsStreamState {
  stream: string;
  version: number;
  event_seq: number;
}

export interface WsEvent {
  kind: "event";
  schema: "event.v1";
  stream: string;
  seq: number;
  version: number;
  hand_index: number;
  name: string;
  payload_schema: number;
  payload: unknown;
}

export interface WsClock {
  kind: "clock";
  schema: "clock.v1";
  stream: string;
  version: number;
  server_time: string;
  seats: WsSeatCountdown[];
}

export interface WsSeatCountdown {
  seat: number;
  remaining_ms: number;
  base_ms: number;
  reserve_ms: number;
}

export interface WsPresence {
  kind: "presence";
  schema: "presence.v1";
  stream: string;
  seats: WsSeatPresence[];
}

export interface WsSeatPresence {
  seat: number;
  online: boolean;
}

export interface WsCommandResult {
  kind: "command_result";
  schema: "command_result.v1";
  command_id: string;
  status: string;
  version: number;
  event_seq: number[];
}

export interface WsError {
  kind: "error";
  schema: "error.v1";
  command_id: string | null;
  code: string;
  message: string;
  retryable: boolean;
}

export type WsServerMessage =
  | WsWelcome
  | WsEvent
  | WsClock
  | WsPresence
  | WsCommandResult
  | WsError
  | { kind: "pong"; server_time: string; latest_seq: number };
