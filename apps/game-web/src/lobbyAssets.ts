import type { LobbyCharacter, ProfileView } from "./types";

export const LOBBY_BACKGROUND = `${import.meta.env.BASE_URL}assets/ui/sakura-campus-empty.png`;
export const FALLBACK_CHARACTER = `${import.meta.env.BASE_URL}assets/local-characters/mahjong-soul/ichihime/outfits/yiji.png`;
export const FALLBACK_AVATAR = `${import.meta.env.BASE_URL}assets/local-characters/mahjong-soul/ichihime/emotes/8.png`;

export interface LobbyPresentation {
  name: string;
  illustrationPath: string;
  avatarPath: string;
}

const fallbackPresentation: LobbyPresentation = {
  name: "一姬",
  illustrationPath: FALLBACK_CHARACTER,
  avatarPath: FALLBACK_AVATAR,
};

/** Pick exactly the same character assets that the lobby will render. */
export function resolveLobbyPresentation(
  characters: LobbyCharacter[],
  profile: ProfileView | null | undefined,
): LobbyPresentation {
  const character =
    characters.find(
      (candidate) => candidate.id === profile?.selected_character?.id,
    ) ??
    characters.find((candidate) => candidate.is_default) ??
    characters[0];

  if (!character) return fallbackPresentation;

  const illustrationPath =
    character.outfits.find(
      (outfit) => outfit.id === profile?.selected_outfit_id,
    )?.illustration_path ?? character.illustration_path;
  const avatarPath =
    character.emotes.find((emote) => emote.path === profile?.avatar_path)
      ?.path ??
    character.emotes.find((emote) => emote.name === "微笑")?.path ??
    character.emotes[0]?.path ??
    illustrationPath;

  return {
    name: character.name,
    illustrationPath,
    avatarPath,
  };
}

const imageLoads = new Map<string, Promise<void>>();

/**
 * Wait until the browser has fetched an image and, where supported, decoded it.
 * Successful promises stay cached so the lobby and its CSS background reuse the
 * exact resources prepared by the splash screen.
 */
export function preloadImage(src: string): Promise<void> {
  const cached = imageLoads.get(src);
  if (cached) return cached;

  const pending = new Promise<void>((resolve, reject) => {
    const image = new Image();
    let settled = false;

    const finish = () => {
      if (settled) return;
      settled = true;
      if (typeof image.decode === "function") {
        void image.decode().catch(() => {}).then(resolve);
      } else {
        resolve();
      }
    };

    image.onload = finish;
    image.onerror = () => {
      if (settled) return;
      settled = true;
      reject(new Error(`图片加载失败：${src}`));
    };
    image.src = src;

    if (image.complete && image.naturalWidth > 0) finish();
  });

  imageLoads.set(src, pending);
  void pending.catch(() => imageLoads.delete(src));
  return pending;
}

export function preloadLobbyImages(
  presentation: LobbyPresentation,
): Promise<void> {
  return Promise.all([
    preloadImage(LOBBY_BACKGROUND),
    preloadImage(presentation.illustrationPath),
    preloadImage(presentation.avatarPath),
  ]).then(() => undefined);
}
