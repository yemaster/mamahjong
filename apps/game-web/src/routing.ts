import { useCallback, useSyncExternalStore } from "react";

export type GameScene =
  | { kind: "lobby" }
  | { kind: "room"; roomId: string }
  | { kind: "create-room" }
  | { kind: "matchmaking"; ticketId: string }
  | { kind: "game"; matchId: string }
  | { kind: "result"; matchId: string }
  | { kind: "profile" };

function parseScene(hash: string): GameScene {
  const path = hash.replace(/^#/, "");
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
    case "profile":
      return { kind: "profile" };
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
    case "profile":
      return "#profile";
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
  const getHash = useCallback(
    () => parseScene(window.location.hash),
    [],
  );
  return useSyncExternalStore(subscribeToHash, getHash);
}
