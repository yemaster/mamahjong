import type {
  AccountStatus,
  AdminCharacter,
  AdminIdentity,
  AuditList,
  CharacterInput,
  CharacterList,
  Overview,
  RoomList,
  SessionBootstrap,
  UserList,
  AdminUser,
  AdminTablecloth,
  TableclothInput,
  TableclothList,
  DatabaseInfo,
  MatchList,
  MatchRecordDetail,
  AdminMusic,
  MusicInput,
  MusicList,
  AssetList,
  ManagedAsset,
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
  init?: RequestInit,
  csrfToken?: string,
): Promise<T> {
  const headers = new Headers(init?.headers);
  if (typeof init?.body === "string") {
    headers.set("content-type", "application/json");
  }
  if (csrfToken) {
    headers.set("x-csrf-token", csrfToken);
  }
  const response = await fetch(`/api/v1/admin${path}`, {
    ...init,
    headers,
    credentials: "same-origin",
  });
  if (!response.ok) {
    if (response.status === 401 && path !== "/me" && path !== "/session") {
      window.dispatchEvent(new Event("mamahjong-admin-unauthorized"));
    }
    const body = (await response.json().catch(() => null)) as {
      code?: string;
      message?: string;
    } | null;
    throw new ApiError(
      body?.message ?? "请求失败",
      response.status,
      body?.code ?? "server.error",
    );
  }
  if (response.status === 204) {
    return undefined as T;
  }
  return (await response.json()) as T;
}

export const adminApi = {
  bootstrap: () => request<SessionBootstrap>("/session"),
  login: (loginName: string, password: string, loginCsrf: string) =>
    request<AdminIdentity>("/session", {
      method: "POST",
      body: JSON.stringify({
        login_name: loginName,
        password,
        login_csrf: loginCsrf,
      }),
    }),
  logout: (csrfToken: string) =>
    request<void>("/session", { method: "DELETE" }, csrfToken),
  identity: () => request<AdminIdentity>("/me"),
  overview: () => request<Overview>("/overview"),
  users: () => request<UserList>("/users"),
  updateUser: (userId: string, nickname: string, csrfToken: string) =>
    request<AdminUser>(
      `/users/${encodeURIComponent(userId)}`,
      { method: "PUT", body: JSON.stringify({ nickname }) },
      csrfToken,
    ),
  updateUserStatus: (
    userId: string,
    status: AccountStatus,
    csrfToken: string,
  ) =>
    request<void>(
      `/users/${encodeURIComponent(userId)}/status`,
      { method: "PUT", body: JSON.stringify({ status }) },
      csrfToken,
    ),
  rooms: () => request<RoomList>("/rooms"),
  matches: () => request<MatchList>("/matches"),
  matchDetail: (matchId: string) =>
    request<MatchRecordDetail>(`/matches/${encodeURIComponent(matchId)}`),
  closeRoom: (roomId: string, csrfToken: string) =>
    request<void>(
      `/rooms/${encodeURIComponent(roomId)}/close`,
      { method: "POST", body: "{}" },
      csrfToken,
    ),
  audit: () => request<AuditList>("/audit"),
  characters: () => request<CharacterList>("/characters"),
  createCharacter: (character: CharacterInput, csrfToken: string) =>
    request<AdminCharacter>(
      "/characters",
      { method: "POST", body: JSON.stringify(character) },
      csrfToken,
    ),
  updateCharacter: (character: CharacterInput, csrfToken: string) =>
    request<AdminCharacter>(
      `/characters/${encodeURIComponent(character.id)}`,
      { method: "PUT", body: JSON.stringify(character) },
      csrfToken,
    ),
  deleteCharacter: (characterId: string, csrfToken: string) =>
    request<void>(
      `/characters/${encodeURIComponent(characterId)}`,
      { method: "DELETE" },
      csrfToken,
    ),
  tablecloths: () => request<TableclothList>("/tablecloths"),
  createTablecloth: (tablecloth: TableclothInput, csrfToken: string) =>
    request<AdminTablecloth>(
      "/tablecloths",
      { method: "POST", body: JSON.stringify(tablecloth) },
      csrfToken,
    ),
  updateTablecloth: (tablecloth: TableclothInput, csrfToken: string) =>
    request<AdminTablecloth>(
      `/tablecloths/${encodeURIComponent(tablecloth.id)}`,
      { method: "PUT", body: JSON.stringify(tablecloth) },
      csrfToken,
    ),
  deleteTablecloth: (tableclothId: string, csrfToken: string) =>
    request<void>(
      `/tablecloths/${encodeURIComponent(tableclothId)}`,
      { method: "DELETE" },
      csrfToken,
    ),
  music: () => request<MusicList>("/music"),
  createMusic: (music: MusicInput, csrfToken: string) =>
    request<AdminMusic>(
      "/music",
      { method: "POST", body: JSON.stringify(music) },
      csrfToken,
    ),
  updateMusic: (music: MusicInput, csrfToken: string) =>
    request<AdminMusic>(
      `/music/${encodeURIComponent(music.id)}`,
      { method: "PUT", body: JSON.stringify(music) },
      csrfToken,
    ),
  deleteMusic: (musicId: string, csrfToken: string) =>
    request<void>(
      `/music/${encodeURIComponent(musicId)}`,
      { method: "DELETE" },
      csrfToken,
    ),
  database: () => request<DatabaseInfo>("/database"),
  assets: (path = "") =>
    request<AssetList>(`/assets?${new URLSearchParams({ path })}`),
  createAssetFolder: (path: string, name: string, csrfToken: string) =>
    request<ManagedAsset>(
      "/assets/folders",
      { method: "POST", body: JSON.stringify({ path, name }) },
      csrfToken,
    ),
  uploadAsset: (path: string, file: File, csrfToken: string) =>
    request<ManagedAsset>(
      `/assets/files?${new URLSearchParams({ path, name: file.name })}`,
      { method: "POST", body: file },
      csrfToken,
    ),
  deleteAsset: (path: string, csrfToken: string) =>
    request<void>(
      `/assets?${new URLSearchParams({ path })}`,
      { method: "DELETE" },
      csrfToken,
    ),
};
