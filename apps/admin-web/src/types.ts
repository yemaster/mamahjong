export type AccountStatus = "active" | "suspended";
export type RoomLifecycle = "waiting" | "playing" | "closed";

export interface SessionBootstrap {
  schema: "admin_session_bootstrap.v1";
  enabled: boolean;
  login_csrf?: string;
}

export interface AdminIdentity {
  schema: "admin_identity.v1";
  id: string;
  login_name: string;
  nickname: string;
  csrf_token: string;
}

export interface Overview {
  schema: "admin_overview.v1";
  user_count: number;
  waiting_room_count: number;
  playing_room_count: number;
  recent_audit: AuditEvent[];
}

export interface AdminUser {
  id: string;
  version: number;
  login_name: string;
  nickname: string;
  status: AccountStatus;
  role: "player" | "administrator";
}

export interface UserList {
  schema: "admin_user_list.v1";
  users: AdminUser[];
}

export interface AdminRoom {
  id: string;
  version: number;
  name: string;
  owner_user_id: string;
  visibility: "public" | "private";
  lifecycle: RoomLifecycle;
  member_count: number;
  seat_count: number;
  active_match_id?: string;
}

export interface RoomList {
  schema: "admin_room_list.v1";
  rooms: AdminRoom[];
}

export interface AuditEvent {
  sequence: number;
  occurred_at: string;
  severity: string;
  category: string;
  action: string;
  actor_id?: string;
  target_type: string;
  target_id?: string;
  outcome: string;
  detail: string;
}

export interface AuditList {
  schema: "admin_audit_list.v1";
  events: AuditEvent[];
}

export interface CharacterAsset {
  name: string;
  path: string;
}

export interface CharacterOutfit {
  id: string;
  name: string;
  illustration_path: string;
}

export interface AdminCharacter {
  id: string;
  version: number;
  name: string;
  illustration_path: string;
  emotes: CharacterAsset[];
  voices: CharacterAsset[];
  outfits: CharacterOutfit[];
  enabled: boolean;
  is_default: boolean;
}

export type CharacterInput = Omit<AdminCharacter, "version">;

export interface CharacterList {
  schema: "admin_character_list.v1";
  characters: AdminCharacter[];
}

export interface AdminTablecloth {
  id: string;
  version: number;
  name: string;
  texture_path: string;
  enabled: boolean;
  is_default: boolean;
}

export type TableclothInput = Omit<AdminTablecloth, "version">;

export interface TableclothList {
  schema: "admin_tablecloth_list.v1";
  tablecloths: AdminTablecloth[];
}
