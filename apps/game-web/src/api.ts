import type {
  ApiFailure,
  AuthResponse,
  CharacterListResponse,
  DefaultCharacterResponse,
  MatchView,
  MatchmakingTicket,
  MusicTrackListResponse,
  RoomList,
  RoomView,
  RuleSetCatalog,
  StartMatchResponse,
  TableclothListResponse,
  UserActivity,
  UserProfileDetail,
  WsTicketResponse,
} from "./types";
import type {
  MatchRecord,
  MatchRecordListResponse,
} from "./replay/recordTypes";

export class ApiError extends Error {
  constructor(
    message: string,
    readonly status: number,
    readonly code: string,
  ) {
    super(message);
  }
}

export const SESSION_INVALID_EVENT = "mamahjong:session-invalid";

async function request<T>(
  path: string,
  init?: RequestInit & { token?: string | null },
): Promise<T> {
  const headers = new Headers(init?.headers);
  if (init?.body) {
    headers.set("content-type", "application/json");
  }
  if (init?.token) {
    headers.set("authorization", `Bearer ${init.token}`);
  }
  const response = await fetch(`/api/v1${path}`, {
    ...init,
    headers,
  });
  if (!response.ok) {
    const body = (await response.json().catch(() => null)) as {
      code?: string;
      message?: string;
    } | null;
    const error = new ApiError(
      body?.message ?? "request failed",
      response.status,
      body?.code ?? "server.unknown",
    );
    if (
      init?.token &&
      response.status === 401 &&
      error.code === "auth.invalid_session"
    ) {
      window.dispatchEvent(new CustomEvent(SESSION_INVALID_EVENT));
    }
    throw error;
  }
  if (response.status === 204) {
    return undefined as T;
  }
  return (await response.json()) as T;
}

export const gameApi = {
  /* ── Auth ──────────────────────────────── */

  register: (loginName: string, password: string, nickname: string) =>
    request<AuthResponse>("/registrations", {
      method: "POST",
      body: JSON.stringify({
        login_name: loginName,
        password,
        nickname,
      }),
    }),

  login: (loginName: string, password: string) =>
    request<AuthResponse>("/sessions", {
      method: "POST",
      body: JSON.stringify({ login_name: loginName, password }),
    }),

  revokeOtherSessions: (token: string) =>
    request<void>("/sessions/me/revoke-others", {
      method: "POST",
      token,
    }),

  me: (token: string) =>
    request<AuthResponse["user"]>("/users/me", { token }),

  activity: (token: string) =>
    request<UserActivity>("/users/me/activity", { token }),

  updateProfile: (token: string, nickname: string) =>
    request<AuthResponse["user"]>("/users/me/profile", {
      method: "PATCH",
      body: JSON.stringify({ nickname }),
      token,
    }),

  updatePresentation: (
    token: string,
    characterId: string,
    outfitId: string,
    avatarPath: string,
  ) =>
    request<AuthResponse["user"]>("/users/me/presentation", {
      method: "PUT",
      body: JSON.stringify({
        character_id: characterId,
        outfit_id: outfitId,
        avatar_path: avatarPath,
      }),
      token,
    }),

  profile: (token: string, userId: string) =>
    request<UserProfileDetail>(`/users/${userId}/profile`, { token }),

  defaultCharacter: () =>
    request<DefaultCharacterResponse>("/characters/default"),

  characters: () => request<CharacterListResponse>("/characters"),

  tablecloths: () => request<TableclothListResponse>("/tablecloths"),

  updateTablecloth: (token: string, tableclothId: string) =>
    request<AuthResponse["user"]>("/users/me/tablecloth", {
      method: "PUT",
      body: JSON.stringify({ tablecloth_id: tableclothId }),
      token,
    }),

  musicTracks: () => request<MusicTrackListResponse>("/music-tracks"),

  updateMusic: (
    token: string,
    selection: { lobby_music_id?: string; match_music_id?: string; riichi_music_id?: string },
  ) =>
    request<AuthResponse["user"]>("/users/me/music", {
      method: "PUT",
      body: JSON.stringify(selection),
      token,
    }),

  /* ── Rules ──────────────────────────────── */

  ruleSets: () => request<RuleSetCatalog>("/rule-sets"),

  /* ── Rooms ──────────────────────────────── */

  rooms: (token: string) => request<RoomList>("/rooms", { token }),

  getRoom: (roomId: string, token: string) =>
    request<RoomView>(`/rooms/${roomId}`, { token }),

  createRoom: (payload: unknown, token: string) =>
    request<RoomView>("/rooms", {
      method: "POST",
      body: JSON.stringify(payload),
      token,
    }),

  joinRoom: (roomId: string, version: number, token: string) =>
    request<RoomView>(`/rooms/${roomId}/members`, {
      method: "POST",
      body: JSON.stringify({ expected_version: version }),
      token,
    }),

  leaveRoom: (roomId: string, version: number, token: string) =>
    request<void>(`/rooms/${roomId}/members`, {
      method: "DELETE",
      body: JSON.stringify({ expected_version: version }),
      token,
    }),

  leaveRoomOnExit: (roomId: string, token: string) => {
    void fetch(`/api/v1/rooms/${roomId}/members/me`, {
      method: "DELETE",
      headers: {
        authorization: `Bearer ${token}`,
      },
      keepalive: true,
    }).catch(() => {});
  },

  setReady: (roomId: string, version: number, ready: boolean, token: string) =>
    request<RoomView>(`/rooms/${roomId}/members/me/readiness`, {
      method: "PUT",
      body: JSON.stringify({ expected_version: version, ready }),
      token,
    }),

  startRoom: (roomId: string, version: number, token: string) =>
    request<StartMatchResponse>(`/rooms/${roomId}/matches`, {
      method: "POST",
      body: JSON.stringify({ expected_version: version }),
      token,
    }),

  /* ── Matchmaking ────────────────────────── */

  enterMatchmaking: (ruleSetId: string, token: string) =>
    request<MatchmakingTicket>("/matchmaking-tickets", {
      method: "POST",
      body: JSON.stringify({ rule_set_id: ruleSetId }),
      token,
    }),

  getTicket: (ticketId: string, token: string) =>
    request<MatchmakingTicket>(`/matchmaking-tickets/${ticketId}`, { token }),

  cancelTicket: (ticketId: string, token: string) =>
    request<MatchmakingTicket>(`/matchmaking-tickets/${ticketId}`, {
      method: "DELETE",
      body: "{}",
      token,
    }),

  /* ── Matches ────────────────────────────── */

  matchView: (matchId: string, token: string) =>
    request<MatchView>(`/matches/${matchId}`, { token }),

  gameCommand: (
    matchId: string,
    version: number,
    name: string,
    payload: unknown,
    token: string,
  ) =>
    request<MatchView>(`/matches/${matchId}/commands`, {
      method: "POST",
      body: JSON.stringify({
        expected_version: version,
        command: { name, payload },
      }),
      token,
    }),

  /* 开发模式：把手牌换成给定牌码。只有开了 MAMAHJONG_DEV_MODE 的服务端才受理。 */
  setDevHand: (matchId: string, tiles: string[], token: string) =>
    request<MatchView>(`/matches/${matchId}/dev/hand`, {
      method: "POST",
      body: JSON.stringify({ tiles }),
      token,
    }),

  /* ── Records ────────────────────────────── */

  records: (token: string) =>
    request<MatchRecordListResponse>("/records", { token }),

  matchRecord: (matchId: string, token: string) =>
    request<MatchRecord>(`/matches/${matchId}/record`, { token }),

  /* ── WebSocket ──────────────────────────── */

  wsTicket: (token: string) =>
    request<WsTicketResponse>("/ws-tickets", {
      method: "POST",
      token,
    }),
};

export function apiFailure(error: unknown): ApiFailure {
  if (error instanceof ApiError) {
    return { code: error.code, message: error.message };
  }
  return {
    code: "client.transport",
    message: error instanceof Error ? error.message : "unknown error",
  };
}
