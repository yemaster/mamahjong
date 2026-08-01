import type {
  ApiFailure,
  AuthResponse,
  MatchView,
  MatchmakingTicket,
  RoomList,
  RoomView,
  RuleSetCatalog,
  StartMatchResponse,
  WsTicketResponse,
} from "./types";

export class ApiError extends Error {
  constructor(
    message: string,
    readonly status: number,
    readonly code: string,
  ) {
    super(message);
  }
}

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
    throw new ApiError(
      body?.message ?? "request failed",
      response.status,
      body?.code ?? "server.unknown",
    );
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

  me: (token: string) =>
    request<AuthResponse["user"]>("/users/me", { token }),

  updateProfile: (token: string, nickname: string) =>
    request<AuthResponse["user"]>("/users/me/profile", {
      method: "PATCH",
      body: JSON.stringify({ nickname }),
      token,
    }),

  /* ── Rules ──────────────────────────────── */

  ruleSets: () => request<RuleSetCatalog>("/rule-sets"),

  /* ── Rooms ──────────────────────────────── */

  rooms: () => request<RoomList>("/rooms"),

  getRoom: (roomId: string) => request<RoomView>(`/rooms/${roomId}`),

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
