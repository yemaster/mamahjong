import { afterEach, describe, expect, it, vi } from "vitest";
import {
  LOBBY_BACKGROUND,
  preloadLobbyImages,
  resolveLobbyPresentation,
} from "./lobbyAssets";
import type { LobbyCharacter, ProfileView } from "./types";

function profile(overrides: Partial<ProfileView> = {}): ProfileView {
  return {
    nickname: "雀士",
    equipped_title: null,
    selected_character: null,
    selected_outfit_id: null,
    avatar_path: null,
    selected_tablecloth_id: null,
    selected_lobby_music_id: null,
    selected_match_music_id: null,
    selected_riichi_music_id: null,
    ranks: [],
    ...overrides,
  };
}

function character(overrides: Partial<LobbyCharacter> = {}): LobbyCharacter {
  return {
    id: "character-a",
    version: 1,
    name: "角色 A",
    illustration_path: "/default.png",
    emotes: [
      { name: "微笑", path: "/smile.png" },
      { name: "得意", path: "/proud.png" },
    ],
    voices: [],
    outfits: [
      { id: "outfit-a", name: "装扮 A", illustration_path: "/outfit.png" },
    ],
    enabled: true,
    is_default: false,
    ...overrides,
  };
}

describe("lobby assets", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("resolves the current user's selected character, outfit and avatar", () => {
    const selected = character();
    const fallback = character({
      id: "default-character",
      name: "默认角色",
      is_default: true,
    });

    expect(
      resolveLobbyPresentation(
        [fallback, selected],
        profile({
          selected_character: { id: selected.id, name: selected.name },
          selected_outfit_id: "outfit-a",
          avatar_path: "/proud.png",
        }),
      ),
    ).toEqual({
      name: "角色 A",
      illustrationPath: "/outfit.png",
      avatarPath: "/proud.png",
    });
  });

  it("waits for the lobby background, illustration and avatar", async () => {
    const images: MockImage[] = [];

    class MockImage {
      onload: (() => void) | null = null;
      onerror: (() => void) | null = null;
      complete = false;
      naturalWidth = 0;
      source = "";

      constructor() {
        images.push(this);
      }

      set src(value: string) {
        this.source = value;
      }
    }

    vi.stubGlobal("Image", MockImage);
    let finished = false;
    const loading = preloadLobbyImages({
      name: "测试角色",
      illustrationPath: "/unique-lobby-illustration.png",
      avatarPath: "/unique-lobby-avatar.png",
    }).then(() => {
      finished = true;
    });

    expect(images.map((image) => image.source)).toEqual([
      LOBBY_BACKGROUND,
      "/unique-lobby-illustration.png",
      "/unique-lobby-avatar.png",
    ]);

    images[0]?.onload?.();
    images[1]?.onload?.();
    await Promise.resolve();
    expect(finished).toBe(false);

    images[2]?.onload?.();
    await loading;
    expect(finished).toBe(true);
  });
});
