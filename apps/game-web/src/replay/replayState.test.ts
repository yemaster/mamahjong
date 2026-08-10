import { describe, expect, it } from "vitest";
import type { HandRecord, RecordEvent } from "./recordTypes";
import { foldHand } from "./replayState";
import { buildSteps, handSteps, handTitle, indexSteps } from "./steps";

/*
 * 一份手搓的四人东一局：发牌、庄家摸打、下家碰、庄家立直、最后荣和。
 * 只放折叠和分步用得上的字段，牌本身随便挑，只要 id 不重就行。
 */

let sequence = 0;

function event(name: string, payload: Record<string, unknown>): RecordEvent {
  sequence += 1;
  return { sequence, name, event_version: 1, payload };
}

function tile(id: number, code: string) {
  return { id, code };
}

/** 十三张起手，id 从 `base` 起连号。 */
function dealt(seat: number, base: number) {
  return event("riichi.initial_hand_dealt", {
    seat,
    tiles: Array.from({ length: 13 }, (_, index) =>
      tile(base + index, `${(index % 9) + 1}m`),
    ),
  });
}

function fixture(): HandRecord {
  sequence = 0;
  const events: RecordEvent[] = [
    event("riichi.hand_started", {
      round_wind: "east",
      round_number: 1,
      dealer: 0,
      honba: 0,
      riichi_sticks: 0,
      dora_indicator: tile(900, "1z"),
      remaining_live_draws: 70,
    }),
    dealt(0, 100),
    dealt(1, 200),
    dealt(2, 300),
    dealt(3, 400),
    /* 第一巡 */
    event("riichi.tile_drawn", {
      seat: 0,
      tile: tile(500, "5p"),
      source: "live_wall",
      remaining_live_draws: 69,
    }),
    event("riichi.furiten_changed", { seat: 0, furiten: false }),
    event("riichi.tile_discarded", {
      seat: 0,
      tile: tile(500, "5p"),
      tsumogiri: true,
      riichi_declared: false,
    }),
    event("riichi.reaction_submitted", { seat: 1 }),
    event("riichi.meld_declared", {
      seat: 1,
      meld: {
        id: 7,
        kind: "pon",
        tiles: [tile(500, "5p"), tile(201, "5p"), tile(202, "5p")],
        called_from: 0,
        called_tile_id: 500,
      },
    }),
    event("riichi.tile_discarded", {
      seat: 1,
      tile: tile(203, "3m"),
      tsumogiri: false,
      riichi_declared: false,
    }),
    event("riichi.tile_drawn", {
      seat: 2,
      tile: tile(501, "6p"),
      source: "live_wall",
      remaining_live_draws: 68,
    }),
    event("riichi.tile_discarded", {
      seat: 2,
      tile: tile(501, "6p"),
      tsumogiri: true,
      riichi_declared: false,
    }),
    event("riichi.tile_drawn", {
      seat: 3,
      tile: tile(502, "7p"),
      source: "live_wall",
      remaining_live_draws: 67,
    }),
    event("riichi.tile_discarded", {
      seat: 3,
      tile: tile(502, "7p"),
      tsumogiri: true,
      riichi_declared: false,
    }),
    /* 第二巡：庄家再摸一次 */
    event("riichi.tile_drawn", {
      seat: 0,
      tile: tile(503, "8p"),
      source: "live_wall",
      remaining_live_draws: 66,
    }),
    event("riichi.tile_discarded", {
      seat: 0,
      tile: tile(503, "8p"),
      tsumogiri: true,
      riichi_declared: true,
    }),
    event("riichi.riichi_established", {
      seat: 0,
      points_after: [24000, 25000, 25000, 25000],
      riichi_sticks: 1,
    }),
    event("riichi.dora_indicator_revealed", {
      tile: tile(901, "2z"),
      revealed_count: 2,
    }),
    event("riichi.ron_declared", { seat: 1, from: 0 }),
  ];

  return {
    hand_index: 0,
    round_wind: "east",
    round_number: 1,
    dealer: 0,
    honba: 0,
    riichi_sticks: 0,
    reason: "ron",
    points_before: [25000, 25000, 25000, 25000],
    point_deltas: [-8000, 8000, 0, 0],
    points_after: [17000, 33000, 25000, 25000],
    winners: [1],
    from: 0,
    tenpai: [],
    nagashi_winners: [],
    awarded_riichi_sticks: 1,
    dealer_continues: false,
    first_event_sequence: 1,
    last_event_sequence: sequence,
    wall: null,
    events,
  };
}

describe("handSteps", () => {
  it("发牌合成一步，折叠到最后一手起手牌", () => {
    const steps = handSteps(fixture(), 0);
    expect(steps[0]).toMatchObject({
      label: "开局",
      turnIndex: 1,
      seat: null,
      eventIndex: 4,
    });
  });

  it("记账事件不产生步骤", () => {
    const labels = handSteps(fixture(), 0).map((step) => step.label);
    expect(labels).toEqual([
      "开局",
      "摸牌",
      "打牌",
      "碰",
      "打牌",
      "摸牌",
      "打牌",
      "摸牌",
      "打牌",
      "摸牌",
      "打牌",
      "立直",
      "新宝牌",
      "荣和",
    ]);
  });

  it("庄家每摸一次牌翻一巡", () => {
    const steps = handSteps(fixture(), 0);
    expect(steps.map((step) => step.turnIndex)).toEqual([
      1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2,
    ]);
  });
});

describe("indexSteps", () => {
  it("记下每一局的起点和步数", () => {
    const hand = fixture();
    const record = {
      schema: "match_record.v1",
      match_id: "m",
      version: 1,
      event_sequence: 0,
      rule_snapshot: {},
      players: [],
      hands: [hand, { ...hand, hand_index: 1, round_number: 2 }],
      result: null,
    };
    const index = indexSteps(buildSteps(record));
    expect(index).toHaveLength(2);
    expect(index[0]).toMatchObject({ handIndex: 0, start: 0, count: 14 });
    expect(index[1]).toMatchObject({ handIndex: 1, start: 14, count: 14 });
  });

  it("每一巡只记第一步", () => {
    const index = indexSteps(handSteps(fixture(), 0));
    expect(index[0]?.turnStarts).toEqual([0, 9]);
  });
});

describe("handTitle", () => {
  it("本场为零就不写本场", () => {
    expect(handTitle(fixture())).toBe("东1局");
  });

  it("有本场就写上", () => {
    expect(handTitle({ ...fixture(), honba: 2 })).toBe("东1局 2本场");
  });
});

describe("foldHand", () => {
  const hand = fixture();
  const steps = handSteps(hand, 0);
  const foldAt = (stepIndex: number) =>
    foldHand(hand, 4, steps[stepIndex]!.eventIndex);

  it("局首前是空桌", () => {
    const state = foldHand(hand, 4, -1);
    expect(state.seats.every((seat) => seat.concealed.length === 0)).toBe(true);
    expect(state.doraIndicators).toEqual([]);
  });

  it("开局那一步四家各十三张", () => {
    const state = foldAt(0);
    expect(state.seats.map((seat) => seat.concealed.length)).toEqual([
      13, 13, 13, 13,
    ]);
    expect(state.doraIndicators).toHaveLength(1);
    expect(state.remainingLiveDraws).toBe(70);
    expect(state.activeSeat).toBeNull();
  });

  it("摸牌那一步手上多一张并记下刚摸的牌", () => {
    const state = foldAt(1);
    expect(state.seats[0]?.concealed).toHaveLength(14);
    expect(state.seats[0]?.drawnTileId).toBe(500);
    expect(state.activeSeat).toBe(0);
    expect(state.remainingLiveDraws).toBe(69);
  });

  it("打牌之后牌进牌河、刚摸的牌清空", () => {
    const state = foldAt(2);
    expect(state.seats[0]?.concealed).toHaveLength(13);
    expect(state.seats[0]?.discards).toHaveLength(1);
    expect(state.seats[0]?.drawnTileId).toBeNull();
    expect(state.activeSeat).toBeNull();
  });

  it("碰：鸣牌者手上少两张，被鸣的那张留在牌河里但标上鸣家", () => {
    const state = foldAt(3);
    expect(state.seats[1]?.melds).toHaveLength(1);
    /* 十三张里出去两张，被鸣的那张本来就不在手上。 */
    expect(state.seats[1]?.concealed).toHaveLength(11);
    expect(state.seats[0]?.discards[0]?.claimed_by).toBe(1);
    expect(state.activeSeat).toBe(1);
  });

  it("立直宣言先挂 pending，落棒之后才是 established", () => {
    const declared = foldAt(10);
    expect(declared.seats[0]?.riichi).toBe("pending");
    expect(declared.progress.riichi_sticks).toBe(0);

    const established = foldAt(11);
    expect(established.seats[0]?.riichi).toBe("established");
    expect(established.progress.riichi_sticks).toBe(1);
  });

  it("翻新宝牌按 revealed_count 接上去", () => {
    const state = foldAt(12);
    expect(state.doraIndicators.map((indicator) => indicator.id)).toEqual([
      900, 901,
    ]);
  });

  it("荣和之后标记终局", () => {
    const state = foldAt(13);
    expect(state.ended).toBe(true);
    expect(state.endReason).toBe("ron");
  });

  it("drawnBy 记全了发牌和摸牌，牌山面板靠它上色", () => {
    const state = foldAt(13);
    /* 四家各十三张起手 + 四次摸牌。 */
    expect(state.drawnBy.size).toBe(4 * 13 + 4);
    expect(state.drawnBy.get(100)).toBe(0);
    expect(state.drawnBy.get(500)).toBe(0);
    expect(state.drawnBy.get(501)).toBe(2);
  });

  it("每一步的四家总张数守恒：暗手 + 副露 + 牌河 = 起手 + 摸牌", () => {
    let drawnSoFar = 0;
    steps.forEach((step, stepIndex) => {
      if (step.label === "摸牌") drawnSoFar += 1;
      const state = foldHand(hand, 4, step.eventIndex);
      let total = 0;
      for (const seat of state.seats) {
        total += seat.concealed.length;
        for (const meld of seat.melds) total += meld.tiles.length;
        total += seat.discards.filter(
          (discard) => discard.claimed_by == null,
        ).length;
      }
      expect(total, `第 ${stepIndex} 步（${step.label}）`).toBe(
        4 * 13 + drawnSoFar,
      );
    });
  });
});
