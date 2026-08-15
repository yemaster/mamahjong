import { useCallback, useMemo, useSyncExternalStore } from "react";

export type GameScene =
  | { kind: "lobby" }
  | { kind: "room"; roomId: string }
  | { kind: "create-room" }
  | { kind: "matchmaking"; ticketId: string }
  | { kind: "game"; matchId: string }
  | { kind: "result"; matchId: string }
  | { kind: "yaku-reference" }
  | { kind: "table-settings" }
  | { kind: "records" }
  | { kind: "replay"; matchId: string }
  | {
      kind: "profile";
      userId?: string;
      tab?: "info" | "character" | "personalization" | "options";
      returnRoomId?: string;
    };

function parseScene(hash: string): GameScene {
  const [path, query = ""] = hash.replace(/^#/, "").split("?");
  if (!path || path === "lobby") {
    return { kind: "lobby" };
  }
  const [segment, id] = path.split("/");
  switch (segment) {
    case "room":
      return { kind: "room", roomId: id! };
    case "create-room":
      return { kind: "create-room" };
    case "matchmaking":
      return { kind: "matchmaking", ticketId: id! };
    case "game":
      return { kind: "game", matchId: id! };
    case "result":
      return { kind: "result", matchId: id! };
    case "yaku-reference":
      return { kind: "yaku-reference" };
    case "table-settings":
      return { kind: "table-settings" };
    case "records":
      return { kind: "records" };
    case "replay":
      return { kind: "replay", matchId: id! };
    case "profile":
      {
        const parameters = new URLSearchParams(query);
        const requestedTab = parameters.get("tab");
        const tab =
          requestedTab === "character" ||
          requestedTab === "personalization" ||
          requestedTab === "options"
            ? requestedTab
            : requestedTab === "interface" || requestedTab === "music"
              ? "personalization"
              : undefined;
        return {
          kind: "profile",
          userId: id || undefined,
          tab,
          returnRoomId: parameters.get("return_room") ?? undefined,
        };
      }
    default:
      return { kind: "lobby" };
  }
}

function sceneHash(scene: GameScene): string {
  switch (scene.kind) {
    case "lobby":
      return "#lobby";
    case "room":
      return `#room/${scene.roomId}`;
    case "create-room":
      return "#create-room";
    case "matchmaking":
      return `#matchmaking/${scene.ticketId}`;
    case "game":
      return `#game/${scene.matchId}`;
    case "result":
      return `#result/${scene.matchId}`;
    case "yaku-reference":
      return "#yaku-reference";
    case "table-settings":
      return "#table-settings";
    case "records":
      return "#records";
    case "replay":
      return `#replay/${scene.matchId}`;
    case "profile":
      {
        const path = scene.userId ? `#profile/${scene.userId}` : "#profile";
        const parameters = new URLSearchParams();
        if (scene.tab && scene.tab !== "info") {
          parameters.set("tab", scene.tab);
        }
        if (scene.returnRoomId) {
          parameters.set("return_room", scene.returnRoomId);
        }
        const query = parameters.toString();
        return query ? `${path}?${query}` : path;
      }
  }
}

export function navigateTo(scene: GameScene): void {
  const hash = sceneHash(scene);
  if (window.location.hash !== hash) {
    window.location.hash = hash;
  }
}

function subscribeToHash(callback: () => void): () => void {
  window.addEventListener("hashchange", callback);
  window.addEventListener("popstate", callback);
  return () => {
    window.removeEventListener("hashchange", callback);
    window.removeEventListener("popstate", callback);
  };
}

export function useGameScene(): GameScene {
  const getHash = useCallback(() => window.location.hash, []);
  const hash = useSyncExternalStore(subscribeToHash, getHash, () => "#lobby");
  return useMemo(() => parseScene(hash), [hash]);
}
