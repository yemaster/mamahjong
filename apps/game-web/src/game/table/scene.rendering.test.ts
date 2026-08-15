import * as THREE from "three";
import { describe, expect, it, vi } from "vitest";
import type { MatchView } from "../../types";
import { tablePreviewView } from "../tablePreviewData";
import type { TableRuntime } from "./types";

vi.mock("./centerConsole", async (importOriginal) => ({
  ...(await importOriginal<typeof import("./centerConsole")>()),
  addCenterConsole: vi.fn(),
}));
vi.mock("./dice", async (importOriginal) => ({
  ...(await importOriginal<typeof import("./dice")>()),
  addTableDice: vi.fn(),
}));
vi.mock("./discards", async (importOriginal) => ({
  ...(await importOriginal<typeof import("./discards")>()),
  addDiscards: vi.fn(),
  addDiscardTile: vi.fn(),
  addNukiTile: vi.fn(),
}));
vi.mock("./hands", async (importOriginal) => {
  const original = await importOriginal<typeof import("./hands")>();
  const three = await import("three");
  return {
    ...original,
    addHand: vi.fn(
      (
        runtime: TableRuntime,
        _view: MatchView,
        player: MatchView["players"][number],
      ) => {
        /* 多放一个隐藏节点模拟真实对手手牌对象池，下一次摸牌直接复用。 */
        for (let index = 0; index <= player.concealed_tile_count; index += 1) {
          const tile = new three.Group();
          const pivot = new three.Group();
          const body = new three.Group();
          pivot.add(body);
          tile.add(pivot);
          tile.position.x = index;
          tile.userData.opponentHandPool = true;
          tile.userData.tileLength = 0.56;
          tile.userData.tileWidth = 0.56 * runtime.tileWidthRatio;
          tile.userData.tilePivot = pivot;
          tile.userData.tileBody = body;
          if (index < player.concealed_tile_count) {
            tile.userData.opponentHandTileIndex = index;
          } else {
            tile.visible = false;
          }
          runtime.renderTarget.add(tile);
        }
      },
    ),
  };
});
vi.mock("./melds", () => ({ addMelds: vi.fn() }));
vi.mock("./selfMotion", () => ({ addSelfDraw: vi.fn() }));
vi.mock("./tableSurface", () => ({ addTableSurface: vi.fn() }));
vi.mock("./tileHighlight", () => ({
  applyTableTileHighlight: vi.fn(),
  rebuildTableTileHighlights: vi.fn(),
}));
vi.mock("./wall", async (importOriginal) => ({
  ...(await importOriginal<typeof import("./wall")>()),
  addWallTile: vi.fn(),
}));

import { renderTable } from "./scene";

function renderingRuntime(): TableRuntime {
  const root = new THREE.Group();
  return {
    root,
    renderTarget: root,
    layers: new Map(),
    pendingLayerDisposals: [],
    tileScale: 1,
    tileWidthRatio: 0.72,
    tileGeometryWidthRatio: 0.72,
    previousView: null,
    latestView: null,
    openingKey: null,
    renderedOpeningPhase: null,
    pendingRinshanDraws: new Map(),
    discardFlights: new Map(),
    openingWallTakeoffs: new Map(),
    openingWallTakeoffKey: null,
    settlementHandKey: null,
    revealedSettlementSeats: new Set(),
    revealedWinningTileSeats: new Set(),
    handCutGaps: new Map(),
    highlightIndexDirty: false,
    dimTsumogiri: false,
    revealAllHands: false,
    instantDraw: false,
    lastDiscard: null,
    animations: [],
    tilts: [],
    spinners: [],
    transients: [],
    impacts: [],
    diceRolls: [],
    selectable: [],
    hovered: null,
    disposed: false,
  } as unknown as TableRuntime;
}

function cloneView(): MatchView {
  return structuredClone(tablePreviewView);
}

function layer(runtime: TableRuntime, key: string): THREE.Group {
  return runtime.layers.get(key)!.group;
}

function seatRiverLayers(
  runtime: TableRuntime,
  seat: number,
): Map<string, THREE.Group> {
  return new Map(
    [...runtime.layers]
      .filter(
        ([key]) =>
          key.startsWith(`discard:${seat}:`) || key.startsWith(`nuki:${seat}:`),
      )
      .map(([key, value]) => [key, value.group]),
  );
}

function appendDiscard(
  view: MatchView,
  seat: number,
  id: number,
  tsumogiri = true,
): void {
  const player = view.players.find((candidate) => candidate.seat === seat)!;
  player.discards.push({
    tile: { id, code: "6m" },
    tsumogiri,
    riichi_declared: false,
    claimed_by: null,
  });
  view.phase = { kind: "awaiting_responses", trigger_seat: seat };
  view.version += 1;
}

describe("牌桌局部渲染边界", () => {
  it("换一家出牌时保留上一家整条牌河，只替换当前牌河和箭头", () => {
    const runtime = renderingRuntime();
    const before = cloneView();
    renderTable(runtime, before, "play", [2, 5], []);

    const firstDiscard = structuredClone(before);
    appendDiscard(firstDiscard, 0, 9001);
    renderTable(runtime, firstDiscard, "play", [2, 5], []);
    const previousRiver = seatRiverLayers(runtime, 0);
    const currentRiverBefore = seatRiverLayers(runtime, 1);
    const untouchedRiver = seatRiverLayers(runtime, 2);
    const markerBefore = layer(runtime, "last-discard-marker");

    const secondDiscard = structuredClone(firstDiscard);
    appendDiscard(secondDiscard, 1, 9002);
    renderTable(runtime, secondDiscard, "play", [2, 5], []);

    for (const [key, group] of previousRiver) {
      expect(layer(runtime, key), key).toBe(group);
    }
    for (const [key, group] of currentRiverBefore) {
      expect(layer(runtime, key), key).toBe(group);
    }
    expect(runtime.layers.has("discard:1:9002")).toBe(true);
    expect(layer(runtime, "last-discard-marker")).not.toBe(markerBefore);
    for (const [key, group] of untouchedRiver) {
      expect(layer(runtime, key), key).toBe(group);
    }
  });

  it("只变化操作选项和版本号时不替换任何 Three.js 图层", () => {
    const runtime = renderingRuntime();
    const before = cloneView();
    renderTable(runtime, before, "play", [2, 5], []);
    const layerGroups = new Map(
      [...runtime.layers].map(([key, value]) => [key, value.group]),
    );

    const after = structuredClone(before);
    after.version += 1;
    after.turn_actions.can_tsumo = !after.turn_actions.can_tsumo;
    renderTable(runtime, after, "play", [2, 5], []);

    for (const [key, group] of layerGroups) {
      expect(layer(runtime, key), key).toBe(group);
    }
  });

  it("摸牌原地复用摸牌者的一张牌背，只替换一个牌山槽位", () => {
    const runtime = renderingRuntime();
    const before = cloneView();
    before.phase = { kind: "awaiting_responses", trigger_seat: 0 };
    renderTable(runtime, before, "play", [2, 5], []);
    const wallLayersBefore = new Map(
      [...runtime.layers]
        .filter(([key]) => key.startsWith("wall-slot:"))
        .map(([key, value]) => [key, value.group]),
    );
    const handsBefore = new Map(
      before.players.map((player) => [player.seat, layer(runtime, `hand:${player.seat}`)]),
    );
    const consoleBefore = layer(runtime, "console");

    const after = structuredClone(before);
    const drawer = after.players.find((player) => player.seat === 1)!;
    drawer.concealed_tile_count += 1;
    drawer.drawn_tile_id = 9901;
    after.phase = { kind: "awaiting_discard", seat: 1 };
    after.remaining_live_draws -= 1;
    after.version += 1;
    renderTable(runtime, after, "play", [2, 5], []);

    const changedWallSlots = [...wallLayersBefore].filter(
      ([key, group]) => layer(runtime, key) !== group,
    );
    expect(changedWallSlots).toHaveLength(1);
    expect(layer(runtime, "hand:1")).toBe(handsBefore.get(1));
    expect(
      layer(runtime, "hand:1").children.filter((object) => object.visible),
    ).toHaveLength(drawer.concealed_tile_count);
    expect(layer(runtime, "hand:0")).toBe(handsBefore.get(0));
    expect(layer(runtime, "hand:2")).toBe(handsBefore.get(2));
    expect(layer(runtime, "hand:3")).toBe(handsBefore.get(3));
    expect(layer(runtime, "console")).toBe(consoleBefore);
  });

  it("杠成立和岭上补摸被合并成一帧时仍从岭上飞一张牌", () => {
    const runtime = renderingRuntime();
    const before = cloneView();
    before.variant_kind = "impact";
    before.completed_rinshan_draws = 2;
    before.phase = { kind: "awaiting_turn_action", seat: 1 };
    renderTable(runtime, before, "play", [2, 5], []);
    const handBefore = layer(runtime, "hand:1");
    runtime.animations = [];
    runtime.tilts = [];

    const after = structuredClone(before);
    const player = after.players.find((candidate) => candidate.seat === 1)!;
    player.melds.push({
      id: 9900,
      kind: "open_kan",
      tiles: [
        { id: 9901, code: "3s" },
        { id: 9902, code: "3s" },
        { id: 9903, code: "3s" },
        { id: 9904, code: "3s" },
      ],
      called_from: 2,
      called_tile_id: 9904,
    });
    /* 四张移入副露、岭上补回一张：中间帧被批处理后净减少三张。 */
    player.concealed_tile_count -= 3;
    after.completed_rinshan_draws = 3;
    after.remaining_live_draws -= 1;
    after.version += 1;

    renderTable(runtime, after, "play", [2, 5], []);

    expect(layer(runtime, "hand:1")).toBe(handBefore);
    expect(runtime.animations).toHaveLength(1);
    expect(runtime.tilts).toHaveLength(1);
    expect(runtime.pendingRinshanDraws.has(1)).toBe(false);
    const animated = runtime.animations[0]!.group;
    expect(animated.visible).toBe(true);
    expect(animated.userData.opponentHandTileIndex).toBe(
      player.concealed_tile_count - 1,
    );
  });

  it("对手手切空隙结束后原地归拢，不二次重建整排手牌", () => {
    vi.useFakeTimers();
    try {
      const runtime = renderingRuntime();
      const before = cloneView();
      before.phase = { kind: "awaiting_discard", seat: 1 };
      renderTable(runtime, before, "play", [2, 5], []);

      const after = structuredClone(before);
      const player = after.players.find((candidate) => candidate.seat === 1)!;
      player.concealed_tile_count -= 1;
      appendDiscard(after, 1, 9003, false);
      renderTable(runtime, after, "play", [2, 5], []);
      const handLayer = layer(runtime, "hand:1");
      const tiles = [...handLayer.children];

      vi.advanceTimersByTime(1_000);

      expect(layer(runtime, "hand:1")).toBe(handLayer);
      expect(handLayer.children).toEqual(tiles);
      expect(runtime.handCutGaps.has(1)).toBe(false);
      expect(runtime.animations).toHaveLength(player.concealed_tile_count);

      const nonVisualUpdate = structuredClone(after);
      nonVisualUpdate.version += 1;
      nonVisualUpdate.turn_actions.can_tsumo = true;
      renderTable(runtime, nonVisualUpdate, "play", [2, 5], []);
      expect(layer(runtime, "hand:1")).toBe(handLayer);
    } finally {
      vi.useRealTimers();
    }
  });

  it("开局 deal、waiting、play 阶段不重复重建四家手牌和牌河", () => {
    const runtime = renderingRuntime();
    const view = cloneView();

    renderTable(runtime, view, "deal", [2, 5], []);
    const dealHands = new Map(
      view.players.map((player) => [player.seat, layer(runtime, `hand:${player.seat}`)]),
    );

    renderTable(runtime, view, "waiting", [2, 5], []);
    const waitingRivers = new Map(
      view.players.map((player) => [
        player.seat,
        seatRiverLayers(runtime, player.seat),
      ]),
    );
    for (const player of view.players) {
      expect(layer(runtime, `hand:${player.seat}`)).toBe(
        dealHands.get(player.seat),
      );
    }

    renderTable(runtime, view, "play", [2, 5], []);
    for (const player of view.players) {
      expect(layer(runtime, `hand:${player.seat}`)).toBe(
        dealHands.get(player.seat),
      );
      for (const [key, group] of waitingRivers.get(player.seat) ?? []) {
        expect(layer(runtime, key), key).toBe(group);
      }
    }
  });
});
