import type { GameCommandName, MatchView } from "../types";
import { canLocalPlayerDiscard } from "./table";

export interface MatchAssistSettings {
  autoSort: boolean;
  autoWin: boolean;
  skipCalls: boolean;
  autoTsumogiri: boolean;
}

export const DEFAULT_MATCH_ASSIST_SETTINGS: MatchAssistSettings = {
  autoSort: true,
  autoWin: false,
  skipCalls: false,
  autoTsumogiri: false,
};

export interface AutomaticMatchCommand {
  name: GameCommandName;
  payload?: unknown;
  delayMs: number;
}

const CALL_REACTIONS = new Set([
  "chi",
  "pon",
  "open_kan",
  "impact_pon",
  "impact_open_kan",
]);

export function automaticMatchCommand(
  view: MatchView,
  settings: MatchAssistSettings,
): AutomaticMatchCommand | null {
  /* 命令名分两套；冲击麻将没有荣和，自动和牌那一支只剩自摸。 */
  const impact = view.variant_kind === "impact";
  const tsumo: GameCommandName = impact ? "impact.tsumo" : "riichi.tsumo";
  const pass: GameCommandName = impact ? "impact.pass" : "riichi.pass";
  const discard: GameCommandName = impact ? "impact.discard" : "riichi.discard";

  if (settings.autoWin) {
    if (view.turn_actions.can_tsumo) {
      return { name: tsumo, delayMs: 180 };
    }
    if (view.available_reactions.some((reaction) => reaction.kind === "ron")) {
      return { name: "riichi.ron", delayMs: 180 };
    }
  }

  const hasRon = view.available_reactions.some(
    (reaction) => reaction.kind === "ron",
  );
  const hasCall = view.available_reactions.some((reaction) =>
    CALL_REACTIONS.has(reaction.kind),
  );
  if (settings.skipCalls && hasCall && !hasRon) {
    return { name: pass, delayMs: 180 };
  }

  const player = view.players.find(
    (candidate) => candidate.seat === view.observer_seat,
  );
  const drawnTileId = player?.drawn_tile_id;
  if (drawnTileId == null || !canLocalPlayerDiscard(view)) {
    return null;
  }

  if (settings.autoTsumogiri) {
    return {
      name: discard,
      payload: { tile_id: drawnTileId },
      delayMs: 460,
    };
  }

  if (
    player?.riichi_status === "established" &&
    !view.turn_actions.can_tsumo
  ) {
    return {
      name: discard,
      payload: { tile_id: drawnTileId },
      delayMs: 460,
    };
  }

  return null;
}

export function loadMatchAssistSettings(
  userId: string | undefined,
): MatchAssistSettings {
  try {
    const stored = window.localStorage.getItem(storageKey(userId));
    if (!stored) return DEFAULT_MATCH_ASSIST_SETTINGS;
    const value = JSON.parse(stored) as Partial<MatchAssistSettings>;
    return {
      autoSort:
        typeof value.autoSort === "boolean" ? value.autoSort : true,
      autoWin: value.autoWin === true,
      skipCalls: value.skipCalls === true,
      autoTsumogiri: value.autoTsumogiri === true,
    };
  } catch {
    return DEFAULT_MATCH_ASSIST_SETTINGS;
  }
}

export function saveMatchAssistSettings(
  userId: string | undefined,
  settings: MatchAssistSettings,
): void {
  try {
    window.localStorage.setItem(storageKey(userId), JSON.stringify(settings));
  } catch {
    // 本地存储不可用时，仅在当前牌局保留设置。
  }
}

function storageKey(userId: string | undefined): string {
  return `mamahjong:match-assist:${userId ?? "local"}`;
}

