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
import { addDiscardTile } from "./discards";
import { addSelfDraw } from "./selfMotion";

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
    coveredWonSeats: new Set(),
    forceHandRebuildSeats: new Set(),
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
  it("四川荣和亮牌时重建放铳牌图层并把胡张标红", () => {
    const runtime = renderingRuntime();
    const view = cloneView();
    view.variant_kind = "sichuan";
    appendDiscard(view, 0, 9901, false);
    const winner = view.players.find((player) => player.seat === 1)!;
    const beforeWin = structuredClone(view);
    beforeWin.phase = { kind: "awaiting_turn_action", seat: 1 };
    winner.won = true;
    winner.drawn_tile_id = null;
    winner.winning_tile = { id: 9901, code: "6m" };

    renderTable(runtime, beforeWin, "play", [2, 5], [], [], null, []);
    const ordinaryLayer = layer(runtime, "discard:0:9901");

    vi.mocked(addDiscardTile).mockClear();
    view.phase = { kind: "awaiting_win_animation", seat: 1 };
    renderTable(runtime, view, "play", [2, 5], [], [], null, [1]);

    expect(layer(runtime, "discard:0:9901")).not.toBe(ordinaryLayer);
    expect(vi.mocked(addDiscardTile)).toHaveBeenCalledWith(
      runtime,
      view,
      expect.objectContaining({ seat: 0 }),
      expect.anything(),
      "play",
      expect.objectContaining({ tile: expect.objectContaining({ id: 9901 }) }),
      expect.any(Number),
      expect.any(Boolean),
      expect.any(Number),
      [{ payerSeat: 0, tileId: 9901 }],
    );
  });

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

  it("自摸视角杠后从岭上摸牌，飞牌动画拿到正确的岭上槽位", () => {
    const runtime = renderingRuntime();
    const before = cloneView();
    before.variant_kind = "impact";
    before.completed_rinshan_draws = 0;
    before.phase = { kind: "awaiting_turn_action", seat: 0 };
    renderTable(runtime, before, "play", [2, 5], []);

    /* 自己暗杠：四张进副露、阶段进 waiting_kan_animation，drawn 暂时不动。 */
    const waiting = structuredClone(before);
    const selfWaiting = waiting.players.find((candidate) => candidate.seat === 0)!;
    selfWaiting.concealed_tiles = selfWaiting.concealed_tiles!.slice(0, -4);
    selfWaiting.concealed_tile_count -= 4;
    selfWaiting.melds.push({
      id: 9900,
      kind: "concealed_kan",
      tiles: [
        { id: 9901, code: "1m" },
        { id: 9902, code: "1m" },
        { id: 9903, code: "1m" },
        { id: 9904, code: "1m" },
      ],
      called_from: null,
      called_tile_id: null,
    });
    waiting.phase = { kind: "awaiting_kan_animation", seat: 0 };
    renderTable(runtime, waiting, "play", [2, 5], []);

    /* 服务端补摸：岭上计数 +1、轮到自己、摸入一张新牌。 */
    const drawn = structuredClone(waiting);
    const selfDrawn = drawn.players.find((candidate) => candidate.seat === 0)!;
    selfDrawn.concealed_tiles = [
      ...(selfDrawn.concealed_tiles ?? []),
      { id: 9999, code: "5m" },
    ];
    selfDrawn.concealed_tile_count += 1;
    selfDrawn.drawn_tile_id = 9999;
    drawn.completed_rinshan_draws = 1;
    drawn.remaining_live_draws -= 1;
    drawn.phase = { kind: "awaiting_turn_action", seat: 0 };
    drawn.version += 1;

    vi.mocked(addSelfDraw).mockClear();
    renderTable(runtime, drawn, "play", [2, 5], []);

    const calls = vi.mocked(addSelfDraw).mock.calls;
    expect(calls.length).toBeGreaterThan(0);
    /* 第 8 个参数是 rinshanDrawNumber。 */
    expect(calls[calls.length - 1]![7]).toBe(1);
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
