import { describe, expect, it } from "vitest";
import { tablePreviewView } from "./tablePreviewData";
import type { ImpactRuleConfig } from "../types";
import {
  automaticMatchCommand,
  DEFAULT_MATCH_ASSIST_SETTINGS,
  resetPerHandMatchAssistSettings,
} from "./matchAssist";

describe("牌局快捷操作", () => {
  it("默认只开启自动理牌", () => {
    expect(DEFAULT_MATCH_ASSIST_SETTINGS).toEqual({
      autoSort: true,
      autoWin: false,
      skipCalls: false,
      autoTsumogiri: false,
    });
  });

  it("新一局关闭和鸣切，但保留理牌选择", () => {
    expect(
      resetPerHandMatchAssistSettings({
        autoSort: false,
        autoWin: true,
        skipCalls: true,
        autoTsumogiri: true,
      }),
    ).toEqual({
      autoSort: false,
      autoWin: false,
      skipCalls: false,
      autoTsumogiri: false,
    });
  });

  it("自动和牌优先于自动摸切", () => {
    const view = {
      ...tablePreviewView,
      turn_actions: {
        ...tablePreviewView.turn_actions,
        can_tsumo: true,
      },
    };
    expect(
      automaticMatchCommand(view, {
        ...DEFAULT_MATCH_ASSIST_SETTINGS,
        autoWin: true,
        autoTsumogiri: true,
      }),
    ).toMatchObject({ name: "riichi.tsumo" });
  });

  it("亮子麻将自动荣和使用冲击麻将指令", () => {
    const view = {
      ...tablePreviewView,
      variant_kind: "impact" as const,
      impact_rules: impactRules("bright"),
      available_reactions: [{ kind: "ron" as const }],
    };

    expect(
      automaticMatchCommand(view, {
        ...DEFAULT_MATCH_ASSIST_SETTINGS,
        autoWin: true,
      }),
    ).toMatchObject({ name: "impact.ron" });
  });

  it("冲击麻将自动和牌只认后端下发的荣和选项", () => {
    const view = {
      ...tablePreviewView,
      variant_kind: "impact" as const,
      impact_rules: impactRules("blind"),
      available_reactions: [{ kind: "ron" as const }],
    };

    expect(
      automaticMatchCommand(view, {
        ...DEFAULT_MATCH_ASSIST_SETTINGS,
        autoWin: true,
      }),
    ).toMatchObject({ name: "impact.ron" });
  });

  it("不鸣只跳过吃碰杠，不会替玩家放弃荣和", () => {
    const callOnly = {
      ...tablePreviewView,
      available_reactions: [
        { kind: "pon" as const, tile_ids: [1, 2] as [number, number] },
      ],
    };
    expect(
      automaticMatchCommand(callOnly, {
        ...DEFAULT_MATCH_ASSIST_SETTINGS,
        skipCalls: true,
      }),
    ).toMatchObject({ name: "riichi.pass" });

    const withRon = {
      ...callOnly,
      available_reactions: [
        ...callOnly.available_reactions,
        { kind: "ron" as const },
      ],
    };
    expect(
      automaticMatchCommand(withRon, {
        ...DEFAULT_MATCH_ASSIST_SETTINGS,
        skipCalls: true,
      }),
    ).toBeNull();
  });

  it("自动摸切始终打出刚摸进的牌", () => {
    const player = tablePreviewView.players.find(
      (candidate) => candidate.seat === tablePreviewView.observer_seat,
    )!;
    const view = {
      ...tablePreviewView,
      phase: {
        kind: "awaiting_discard" as const,
        seat: tablePreviewView.observer_seat,
      },
      players: tablePreviewView.players.map((candidate) =>
        candidate.seat === player.seat
          ? {
              ...candidate,
              drawn_tile_id:
                candidate.concealed_tiles?.at(-1)?.id ?? null,
            }
          : candidate,
      ),
    };
    expect(
      automaticMatchCommand(view, {
        ...DEFAULT_MATCH_ASSIST_SETTINGS,
        autoTsumogiri: true,
      }),
    ).toMatchObject({
      name: "riichi.discard",
      payload: { tile_id: player.concealed_tiles?.at(-1)?.id },
    });
  });

  it("立直后非胡牌在手中停留一秒再摸切", () => {
    const view = {
      ...tablePreviewView,
      phase: {
        kind: "awaiting_discard" as const,
        seat: tablePreviewView.observer_seat,
      },
      players: tablePreviewView.players.map((player) =>
        player.seat === tablePreviewView.observer_seat
          ? {
              ...player,
              riichi_status: "established" as const,
              drawn_tile_id: player.concealed_tiles?.at(-1)?.id ?? null,
            }
          : player,
      ),
    };

    expect(
      automaticMatchCommand(view, DEFAULT_MATCH_ASSIST_SETTINGS),
    ).toMatchObject({ name: "riichi.discard", delayMs: 1000 });
  });
});

function impactRules(mode: ImpactRuleConfig["mode"]): ImpactRuleConfig {
  const bright = mode === "bright";
  return {
    mode,
    match_rules: { thinking_time: { base_seconds: 5, reserve_seconds: 20 } },
    kan: {
      added_kan_single_payer: !bright,
      indicator_pon_counts_as_kan: !bright,
      first_round_repeat_discard: !bright,
      four_identical_discards_as_kan: !bright,
      pon_with_few_tiles_as_kan: !bright,
    },
    special: { seven_gaps: false },
    all_in: {
      eleven_honor_streak: true,
      all_honors: !bright,
      pure_flush_no_joker: !bright,
      single_wait: !bright,
      three_kans: !bright,
      four_jokers: true,
      pure_seven_pairs: !bright,
      last_tile: !bright,
      blessing: true,
    },
  };
}
