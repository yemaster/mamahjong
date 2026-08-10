import { describe, expect, it } from "vitest";
import { tablePreviewView } from "./tablePreviewData";
import {
  automaticMatchCommand,
  DEFAULT_MATCH_ASSIST_SETTINGS,
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
});
