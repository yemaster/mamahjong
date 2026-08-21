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

/**
 * 新一局开始时，可能替玩家直接发送指令的三项必须重新关闭。自动理牌只改变本地
 * 展示顺序，不会替玩家操作，所以保留玩家上一局的选择。
 */
export function resetPerHandMatchAssistSettings(
  settings: MatchAssistSettings,
): MatchAssistSettings {
  if (!settings.autoWin && !settings.skipCalls && !settings.autoTsumogiri) {
    return settings;
  }
  return {
    ...settings,
    autoWin: false,
    skipCalls: false,
    autoTsumogiri: false,
  };
}

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
  /* 动作是否合法完全由后端下发；前端只根据麻将种类选择命名空间。 */
  const impact = view.variant_kind === "impact";
  const tsumo: GameCommandName = impact ? "impact.tsumo" : "riichi.tsumo";
  const pass: GameCommandName = impact ? "impact.pass" : "riichi.pass";
  const discard: GameCommandName = impact ? "impact.discard" : "riichi.discard";

  if (settings.autoWin) {
    if (view.turn_actions.can_tsumo) {
      return { name: tsumo, delayMs: 180 };
    }
    if (view.available_reactions.some((reaction) => reaction.kind === "ron")) {
      return { name: impact ? "impact.ron" : "riichi.ron", delayMs: 180 };
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
      delayMs: 1000,
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
