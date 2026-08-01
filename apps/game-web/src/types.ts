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

/* ── Rules ─────────────────────────────────────────────────── */

export interface RuleSetSummary {
  id: string;
  name: string;
  seat_count: number;
}

export interface RuleSetCatalog extends ApiEnvelope {
  rule_sets: RuleSetDetail[];
}

export interface RuleSetDetail {
  id: string;
  name: string;
  seat_count: number;
  presets: RulePreset[];
  default_config: unknown;
}

export interface RulePreset {
  id: string;
  revision: number;
  display_name: string;
}

/* ── Rooms ─────────────────────────────────────────────────── */

export interface RoomList extends ApiEnvelope {
  rooms: RoomView[];
}

export interface RoomView extends ApiEnvelope {
  id: string;
  version: number;
  owner_user_id: string;
  name: string;
  visibility: "public" | "private";
  lifecycle: "waiting" | "playing" | "closed";
  rule_snapshot: unknown;
  members: RoomMemberView[];
  active_match_id: string | null;
}

export interface RoomMemberView {
  user_id: string;
  seat: number;
  nickname: string;
  ready: boolean;
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

export interface MatchView extends ApiEnvelope {
  id: string;
  room_id: string;
  version: number;
  event_sequence: number;
  hand_index: number;
  observer_seat: number;
  progress: ProgressView;
  phase: MatchPhase;
  remaining_live_draws: number;
  dora_indicators: TileView[];
  players: MatchPlayerView[];
  available_reactions: ReactionOption[];
  turn_actions: TurnActions;
  clocks: SeatClockView[];
  result: MatchResult | null;
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
  points: number;
  concealed_tiles: TileView[] | null;
  concealed_tile_count: number;
  drawn_tile_id: number | null;
  melds: MeldView[];
  discards: DiscardView[];
  riichi_status: "none" | "pending" | "established";
}

export interface TileView {
  id: number;
  code: string;
}

export interface MeldView {
  id: number;
  kind: "chi" | "pon" | "open_kan" | "concealed_kan" | "added_kan";
  tiles: TileView[];
  called_from: number | null;
  called_tile_id: number | null;
}

export interface DiscardView {
  tile: TileView;
  tsumogiri: boolean;
  riichi_declared: boolean;
  claimed_by: number | null;
}

export type ReactionOption =
  | { kind: "ron" }
  | { kind: "chi"; tile_ids: [number, number] }
  | { kind: "pon"; tile_ids: [number, number] }
  | { kind: "open_kan"; tile_ids: [number, number, number] };

export interface TurnActions {
  can_tsumo: boolean;
  riichi_discard_tile_ids: number[];
  concealed_kan_tile_ids: [number, number, number, number][];
  added_kan_options: AddedKanOption[];
  can_nine_terminals: boolean;
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
  end_reason: string;
  hand_count: number;
  final_points: number[];
  placements: Placement[];
  unclaimed_riichi_sticks_awarded: number;
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
  | "riichi.added_kan";

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
