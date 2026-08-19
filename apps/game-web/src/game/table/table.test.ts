import { describe, expect, it } from "vitest";
import * as THREE from "three";
import type { DiscardView, MatchView } from "../../types";
import { tablePreviewView } from "../tablePreviewData";
import {
  addedKanTilePosition,
  billboardHandTilt,
  canLocalPlayerDiscard,
  completedImpactRinshanDraws,
  countCompletedKans,
  coveredHandTilt,
  doraWallTileIndex,
  discardGridPosition,
  discardNaturalRotation,
  handPosition,
  exchangeStackPosition,
  exchangeRecipient,
  impactWallLayout,
  impactWallTiles,
  isSichuanOpeningDealerDraw,
  meldDisplayTiles,
  meldTilePosition,
  nukiRiverPosition,
  openingDealArrival,
  openingDealDuration,
  openingDealOrder,
  openingDealStep,
  openingWallTakeoffSchedule,
  opponentHandLayout,
  orthographicCameraBounds,
  playerIsHoldingDrawnTile,
  playerSichuanWinIsTsumo,
  playerCompletedKan,
  playerExtractedNorth,
  playerReceivedDraw,
  resolveRinshanDrawNumber,
  riichiWallLayout,
  riichiWallTiles,
  sanmaWallLayout,
  riverDiscardEntries,
  rinshanWallSlot,
  screenRectAnchor,
  settlementCoveringSeats,
  settlementFallEase,
  settlementHandShift,
  settlementHandTilt,
  meldPushSource,
  MELD_PUSH_MS,
  sortHandForDisplay,
  standUpEase,
  standingHandTilt,
  TILE_LENGTH,
  TILE_STAND_UP_MS,
  TILE_WIDTH_RATIO,
  tsumoThrowArc,
  tsumoThrowEase,
  tableCameraLayout,
  tableLayoutZones,
  tableRelativeSeat,
  wallBreakSlot,
  wallTileOrigin,
  wallTileQuaternion,
  winningTileIndex,
} from "./index";
import { doraCodeForIndicator, isDoraTile } from "../tileAssets";
import {
  advanceCameraShake,
  advanceTableImpacts,
  cameraShakeOffset,
} from "./impact";
import type { TableImpact, TableRuntime } from "./types";
import {
  HAND_TILE_GAP,
  OPENING_DEAL_STEP_MS,
  RIVER_TILE_LENGTH,
  WALL_DISTANCE,
  WALL_TILE_LENGTH,
} from "./constants";

describe("手牌理牌", () => {
  it("按万筒索字排序并把摸入牌留在末端", () => {
    const tiles = [
      { id: 1, code: "7z" },
      { id: 2, code: "3p" },
      { id: 3, code: "1m" },
      { id: 4, code: "2s" },
      { id: 5, code: "5m" },
    ];

    expect(sortHandForDisplay(tiles, 2).map((tile) => tile.id)).toEqual([
      3,
      5,
      4,
      1,
      2,
    ]);
  });

  it("冲击麻将的自摸牌即使是财神也留在最右侧", () => {
    const tiles = [
      { id: 1, code: "7z" },
      { id: 2, code: "3p" },
      { id: 3, code: "1m" },
      { id: 4, code: "3p" },
    ];

    expect(sortHandForDisplay(tiles, 4, "3p").map((tile) => tile.id)).toEqual([
      2,
      3,
      1,
      4,
    ]);
  });
});

describe("副露来源方向", () => {
  const meld = {
    id: 1,
    kind: "pon" as const,
    tiles: [
      { id: 11, code: "5m" },
      { id: 12, code: "5m" },
      { id: 13, code: "5m" },
    ],
    called_from: 1,
    called_tile_id: 12,
  };

  /* 返回的数组从右往左排，所以下家那张要落在第 0 位。 */
  it("下家的牌放在副露右端并朝向下家", () => {
    const displayed = meldDisplayTiles(meld, 1);
    expect(displayed.map(({ tile }) => tile.id)).toEqual([12, 11, 13]);
    expect(displayed[0]!.calledRotation).toBe(Math.PI / 2);
  });

  it("对家的牌放在副露中间", () => {
    const displayed = meldDisplayTiles(meld, 2);
    expect(displayed.map(({ tile }) => tile.id)).toEqual([11, 12, 13]);
    expect(displayed[1]!.calledRotation).toBe(Math.PI / 2);
  });

  /* 四张牌时「中间」得说清楚是哪一张：从左数第二张，也就是数组的倒数第二位。 */
  it("杠对家时横置牌落在从左数第二张", () => {
    const openKan = {
      ...meld,
      kind: "open_kan" as const,
      tiles: [...meld.tiles, { id: 14, code: "5m" }],
    };
    const displayed = meldDisplayTiles(openKan, 2);
    expect(displayed.map(({ tile }) => tile.id)).toEqual([11, 13, 12, 14]);
    expect(displayed[2]!.calledRotation).toBe(Math.PI / 2);
  });

  it("暗杠四张直立，两端两张扣着", () => {
    const concealedKan = {
      ...meld,
      kind: "concealed_kan" as const,
      tiles: [...meld.tiles, { id: 14, code: "5m" }],
      called_from: null,
      called_tile_id: null,
    };
    const displayed = meldDisplayTiles(concealedKan, null);
    expect(displayed.map(({ faceDown }) => faceDown)).toEqual([
      true,
      false,
      false,
      true,
    ]);
    expect(
      displayed.every(({ calledRotation }) => calledRotation === 0),
    ).toBe(true);
  });

  it("上家的牌放在副露左端并朝向上家", () => {
    const displayed = meldDisplayTiles(meld, 3);
    expect(displayed.map(({ tile }) => tile.id)).toEqual([13, 11, 12]);
    expect(displayed[2]!.calledRotation).toBe(-Math.PI / 2);
  });

  it("加杠第四张在桌面平放并紧贴横置采用牌且不占横向位置", () => {
    const addedKan = {
      ...meld,
      kind: "added_kan" as const,
      tiles: [...meld.tiles, { id: 14, code: "5m" }],
    };
    const displayed = meldDisplayTiles(addedKan, 1);
    expect(displayed.map(({ tile }) => tile.id)).toEqual([12, 14, 11, 13]);
    expect(
      displayed.filter(({ addedBesideCalled }) => addedBesideCalled),
    ).toHaveLength(1);
    expect(displayed[1]).toMatchObject({
      calledRotation: Math.PI / 2,
      addedBesideCalled: true,
    });
    const calledPosition = meldTilePosition(0, 0, true);
    const addedPosition = addedKanTilePosition(calledPosition, 0);
    expect(addedPosition.y).toBe(calledPosition.y);
    expect(addedPosition.z).toBeLessThan(calledPosition.z);
  });

  it("普通明杠四张牌保持单层排列", () => {
    const openKan = {
      ...meld,
      kind: "open_kan" as const,
      tiles: [...meld.tiles, { id: 14, code: "5m" }],
    };
    const displayed = meldDisplayTiles(openKan, 3);
    expect(displayed).toHaveLength(4);
    expect(
      displayed.every(({ addedBesideCalled }) => !addedBesideCalled),
    ).toBe(true);
    expect(displayed.filter(({ calledRotation }) => calledRotation !== 0)).toHaveLength(1);
  });

  it("四个方向的副露起点均位于桌布安全区", () => {
    const positions = [0, 1, 2, 3].map((relative) =>
      meldTilePosition(relative, 0, false),
    );
    for (const position of positions) {
      expect(Math.abs(position.x)).toBeLessThanOrEqual(5.63);
      expect(Math.abs(position.z)).toBeLessThanOrEqual(5.63);
    }
    expect(positions[0]!.x).toBeGreaterThan(0);
    expect(positions[1]!.z).toBeLessThan(0);
    expect(positions[2]!.x).toBeLessThan(0);
    expect(positions[3]!.z).toBeGreaterThan(0);
  });

  it("横放鸣牌靠近玩家的一侧与竖放牌底边对齐", () => {
    const upright = meldTilePosition(0, 0, false);
    const rotated = meldTilePosition(0, 0, true);
    expect(rotated.z).toBeGreaterThan(upright.z);
  });

  it("牌河与牌山分层，手牌和副露共用对称玩家基线", () => {
    const zones = tableLayoutZones();
    expect(zones.tile.width).toBeCloseTo(0.387072);
    expect(zones.tile.length).toBeCloseTo(0.5376);
    expect(zones.tile.depth).toBeCloseTo(0.2688);
    expect(zones.wall.inner - zones.river.outer).toBeGreaterThanOrEqual(0.65);
    expect(zones.meld.inner - zones.wall.outer).toBeGreaterThanOrEqual(0.65);
    expect(zones.hand.center).toBe(zones.meld.center);
    expect(
      zones.table.clothHalfSide - zones.meld.outer,
    ).toBeGreaterThanOrEqual(0.35);
    expect(zones.meld.rightEdge).toBe(4.6);
  });

  it("四组明杠的最坏宽度旋转到任一座位后仍留在桌内", () => {
    const lastTileCursor = 6;
    for (const relative of [0, 1, 2, 3]) {
      const first = meldTilePosition(relative, 0, false);
      const last = meldTilePosition(relative, lastTileCursor, true);
      for (const position of [first, last]) {
        expect(Math.abs(position.x)).toBeLessThanOrEqual(5.63);
        expect(Math.abs(position.z)).toBeLessThanOrEqual(5.63);
      }
    }
  });
});

describe("开门位置", () => {
  /* 骰子点数从庄家数起（庄家为 1，往下家方向数），数到谁就拆谁的墙。 */
  it("按骰子点数从庄家开始数到该拆的那面墙", () => {
    /* 庄家在本家：1 点拆自己，2 点拆下家，3 点拆对家，4 点又回到上家。 */
    expect(wallTileQuaternion(wallBreakSlot(0, 1)).angleTo(quarterTurn(0)))
      .toBeCloseTo(0);
    expect(wallTileQuaternion(wallBreakSlot(0, 2)).angleTo(quarterTurn(1)))
      .toBeCloseTo(0);
    expect(wallTileQuaternion(wallBreakSlot(0, 3)).angleTo(quarterTurn(2)))
      .toBeCloseTo(0);
    expect(wallTileQuaternion(wallBreakSlot(0, 4)).angleTo(quarterTurn(3)))
      .toBeCloseTo(0);
    /* 庄家在对家时同样从庄家数起，7 点落在庄家的对家、也就是本家这面。 */
    expect(wallTileQuaternion(wallBreakSlot(2, 7)).angleTo(quarterTurn(0)))
      .toBeCloseTo(0);
  });

  it("留下的墩数正好等于骰子点数", () => {
    /* 7 点：右边留 7 墩不动，从第 8 墩上面那张开始摸。 */
    for (const [dealer, dice, kept] of [
      [0, 7, 7],
      [1, 3, 3],
      [2, 11, 11],
    ] as const) {
      const slot = wallBreakSlot(dealer, dice);
      expect(slot % 2).toBe(0);
      expect(Math.floor(slot / 2) % 17).toBe(kept);
    }
  });

  it("开门位置只跟庄家和点数有关，跟观察者无关", () => {
    expect(wallBreakSlot(1, 5)).toBe(wallBreakSlot(1, 5));
    expect(wallBreakSlot(0, 5)).not.toBe(wallBreakSlot(1, 5));
  });
});

describe("牌山增量槽位", () => {
  it("立直麻将正常摸一张时只少一个物理槽位", () => {
    const layout = riichiWallLayout(0, [2, 5]);
    const before = riichiWallTiles(layout, 70, [], 0, false);
    const after = riichiWallTiles(layout, 69, [], 0, false);
    const afterSlots = new Set(after.map((tile) => tile.slot));

    expect(before).toHaveLength(after.length + 1);
    expect(before.filter((tile) => !afterSlots.has(tile.slot))).toHaveLength(1);
    expect(after.every((tile) => before.some((old) => old.slot === tile.slot)))
      .toBe(true);
  });

  it("冲击麻将正常摸一张时也只少一个物理槽位", () => {
    const layout = impactWallLayout(0, 0, [2, 5]);
    const before = impactWallTiles(layout, 70, 0, undefined);
    const after = impactWallTiles(layout, 69, 0, undefined);
    const afterSlots = new Set(after.map((tile) => tile.slot));

    expect(before).toHaveLength(after.length + 1);
    expect(before.filter((tile) => !afterSlots.has(tile.slot))).toHaveLength(1);
  });

  it("开局包含主视角的牌都等轮到起飞时才从牌山隐藏", () => {
    const dealer = 0;
    const observerSeat = 2;
    const layout = riichiWallLayout(dealer, [2, 5]);
    const players = [0, 1, 2, 3].map((seat) => ({
      seat,
      concealed_tile_count: seat === dealer ? 14 : 13,
    }));
    const startedAt = 10_000;
    const schedule = openingWallTakeoffSchedule(
      layout,
      players,
      dealer,
      players.length,
      startedAt,
    );
    const selfFirstIndex = openingDealOrder(
      0,
      observerSeat,
      dealer,
      players.length,
    );
    const selfFirstStep = openingDealStep(
      0,
      observerSeat,
      dealer,
      players.length,
    );

    expect(schedule).toHaveLength(53);
    expect(schedule.get(layout.drawSlot(selfFirstIndex))).toBe(
      startedAt + selfFirstStep * OPENING_DEAL_STEP_MS,
    );
  });
});

function quarterTurn(turns: number): THREE.Quaternion {
  return new THREE.Quaternion().setFromAxisAngle(
    new THREE.Vector3(0, 1, 0),
    turns * (Math.PI / 2),
  );
}

describe("冲击麻将的牌山", () => {
	  /* 冲击麻将的槽位直接就是物理位置：相对座次 * 34 + 墩号 * 2 + 层。
	   * 相对座次 0=自家(屏幕下) 1=下家(右) 2=对家(上) 3=上家(左)，
	   * 墩号从这一家的左手边 0 数到右手边 16。中间不能再做镜像翻面。 */
	  function stackOf(slot: number): { seat: number; stack: number } {
	    return { seat: Math.floor(slot / 34), stack: Math.floor(slot / 2) % 17 };
	  }

	  it("拆庄家对家的墙，从右边预留墩的左侧起往左摸", () => {
	    /* 庄家 0，观察者 0，[2, 5]：和 7 → 割目家 = (0+7-1)%4 = 2（对家）、
	       右边预留 2 墩（第 15、16 墩），起点是紧挨着的第 14 墩。 */
	    const layout = impactWallLayout(0, 0, [2, 5]);
	    expect(stackOf(layout.drawSlot(0))).toEqual({ seat: 2, stack: 14 });
	    /* 立直同样的骰子留 7 墩，两家不是一套规矩。 */
	    expect(layout.drawSlot(0)).not.toBe(
	      riichiWallLayout(0, [2, 5]).drawSlot(0),
	    );
	  });

	  it("摸牌从右往左、先上后下，走完一面顺时针接上家那面的右端", () => {
	    /* [1, 1]：和 2 → 割目家 = (0+2-1)%4 = 1（下家）、右边预留 1 墩（第 16 墩）。
	       起点第 15 墩往左摸到第 0 墩 → 上家 → 对家 → 下家 → 割目家预留的第 16 墩。 */
	    const layout = impactWallLayout(0, 0, [1, 1]);
	    expect(stackOf(layout.drawSlot(0))).toEqual({ seat: 1, stack: 15 });
	    /* 同一墩下层紧跟着上层。 */
	    expect(stackOf(layout.drawSlot(1))).toEqual({ seat: 1, stack: 15 });
	    /* 割目家这 16 墩走完（32 张），顺时针接上家那面墙的最右墩。 */
	    expect(stackOf(layout.drawSlot(31))).toEqual({ seat: 1, stack: 0 });
	    expect(stackOf(layout.drawSlot(32))).toEqual({ seat: 0, stack: 16 });
	  });

	  it("顺时针走的是「割目家 → 上家 → 对家 → 下家」而不是反过来", () => {
	    /* [1, 1]：割目家 seat 1，预留 1 墩。四面墙各 17 墩，
	       但割目家开头只有 16 墩，所以后面三面的起点分别是 32、66、100 张之后。 */
	    const layout = impactWallLayout(0, 0, [1, 1]);
	    const sideAt = (order: number) => stackOf(layout.drawSlot(order)).seat;
	    /* 割目家 seat 1 的上家是 seat 0，再上家 seat 3，再上家 seat 2。 */
	    expect([sideAt(0), sideAt(32), sideAt(66), sideAt(100)]).toEqual([
	      1, 0, 3, 2,
	    ]);
	  });

	  it("财神指示牌在割目家逆时针下家那面墙、从左数第 x+y 墩的上层", () => {
	    /* 开门在 seat 2（对家），翻财神的是它的逆时针**下家** seat 3（上家 / 左墙）；
	       从左往右第 7 墩，墩号 0 起算就是 6。 */
	    const layout = impactWallLayout(0, 0, [2, 5]);
	    expect(layout.revealedSlot).not.toBeNull();
	    expect(stackOf(layout.revealedSlot!)).toEqual({ seat: 3, stack: 6 });
	    expect(layout.revealedSlot! % 2).toBe(0);
	    expect(layout.deadSlots).toEqual([
	      layout.revealedSlot!,
	      layout.revealedSlot! + 1,
	    ]);
	  });

	  it("摸牌路线绕桌一圈是顺时针，中途不回头", () => {
	    /* 不看槽位算式，直接看摸牌落点在桌面上怎么转：
	       θ = atan2(x, z)，自家墙 θ≈0、下家(右)墙 θ≈+90°，所以 θ 递增是逆时针。
	       顺时针摸 = θ 一路不增，整圈累计正好 -2π。 */
	    for (const dice of [[1, 1], [2, 5], [3, 4], [6, 6]] as [
	      number,
	      number,
	    ][]) {
	      const layout = impactWallLayout(0, 0, dice);
	      const angles = Array.from(
	        { length: layout.drawableCount },
	        (_, order) => {
	          const p = layout.origin(layout.drawSlot(order), 0.75, 1);
	          return Math.atan2(p.x, p.z);
	        },
	      );
	      let total = 0;
	      for (let i = 1; i < angles.length; i += 1) {
	        let delta = angles[i]! - angles[i - 1]!;
	        /* 跨过 ±π 那一下要绕回来。 */
	        if (delta > Math.PI) delta -= 2 * Math.PI;
	        if (delta < -Math.PI) delta += 2 * Math.PI;
	        expect(delta).toBeLessThanOrEqual(1e-9);
	        total += delta;
	      }
	      /* 首尾差一墩没接上（还跳掉了财神那墩），所以到不了整整 -2π。 */
      expect(total).toBeLessThan(-2 * Math.PI + 0.2);
      expect(total).toBeGreaterThan(-2 * Math.PI);
	    }
	  });

	  it("财神那一墩落在整条牌山的倒数第 2*(x+x+y)-1 张，和后端对得上", () => {
	    /* 后端 `indicator_offset_from_end` = 2*(x + x+y) - 1，
	       换算成墩号就是从牌山开头数第 68 - (x + x+y) 墩。
	       这里把跳过的那一墩塞回摸牌序列，看它排第几。 */
	    for (let a = 1; a <= 6; a += 1) {
	      for (let b = 1; b <= 6; b += 1) {
	        for (let dealer = 0; dealer < 4; dealer += 1) {
	          const dice: [number, number] = [a, b];
	          const layout = impactWallLayout(dealer, 0, dice);
	          const drawn = Array.from(
	            { length: layout.drawableCount },
	            (_, order) => layout.drawSlot(order),
	          );
	          /* 财神墩后面紧接着摸的那一张，在完整牌山里排在它之后一墩。
	             整条路每面墙内墩号递减，所以序列里第一个「墩号比财神墩小、
	             或者换了面墙」的位置就是断点。 */
	          const dead = layout.revealedSlot!;
	          const deadStack = Math.floor(dead / 2);
	          const cut = drawn.findIndex(
	            (slot) => Math.floor(slot / 2) === deadStack - 1,
	          );
	          /* cut 是完整牌山里财神墩之后那一墩的第一张，
	             它前面有 cut 张可摸牌 + 财神那墩 2 张。 */
	          expect(cut % 2).toBe(0);
	          expect(cut / 2).toBe(68 - (Math.min(a, b) + a + b));
	        }
	      }
	    }
	  });

	  it("摸牌整墩跳过翻财神那一墩，可摸 134 张", () => {
	    /* [3, 4]：和 7 → 割目家 = (1+7-1)%4 = 3（上家） */
	    const layout = impactWallLayout(1, 1, [3, 4]);
	    expect(layout.drawableCount).toBe(134);
	    const drawn = Array.from(
	      { length: layout.drawableCount },
	      (_, order) => layout.drawSlot(order),
	    );
	    expect(new Set(drawn).size).toBe(134);
	    for (const dead of layout.deadSlots) {
	      expect(drawn).not.toContain(dead);
	    }
	  });

	  it("杠张从末尾一墩一墩往回取，每墩先上层后下层", () => {
	    /* [6, 6]：和 12 → 割目家 = (2+12-1)%4 = 1（下家） */
	    const layout = impactWallLayout(2, 2, [6, 6]);
	    expect(layout.rinshanSlot(1)).toBe(layout.drawSlot(132));
	    expect(layout.rinshanSlot(2)).toBe(layout.drawSlot(133));
	    expect(layout.rinshanSlot(3)).toBe(layout.drawSlot(130));
    expect(layout.rinshanSlot(4)).toBe(layout.drawSlot(131));
    /* 上层的槽位是偶数：先摸到的确实是靠上那张。 */
    expect(layout.rinshanSlot(1) % 2).toBe(0);
    expect(layout.rinshanSlot(2) % 2).toBe(1);
    expect(layout.deadSlots).not.toContain(layout.rinshanSlot(1));
	  });
	});


describe("杠后岭上摸牌", () => {
  it("杠和拔北共用岭上序列，每墩都先取上层再取下层", () => {
    expect(Array.from({ length: 4 }, (_, index) => rinshanWallSlot(42, index + 1))).toEqual([
      40, 41, 38, 39,
    ]);

    const layout = riichiWallLayout(0, [1, 1]);
    expect(Array.from({ length: 4 }, (_, index) => layout.rinshanOrderIndex(index + 1))).toEqual([
      134, 135, 132, 133,
    ]);
  });

  it("预览中的两组杠会在牌山末尾留下两个空位", () => {
    expect(countCompletedKans(tablePreviewView)).toBe(2);
  });

  it("碰升级为加杠和新增普通杠都会识别为岭上摸牌", () => {
    const current = tablePreviewView.players.find(
      (player) => player.seat === 3,
    )!;
    const previous = {
      ...current,
      melds: [
        {
          ...current.melds[0]!,
          kind: "pon" as const,
          tiles: current.melds[0]!.tiles.slice(0, 3),
        },
      ],
    };
    expect(playerCompletedKan(current, previous)).toBe(true);
  });

  it("杠动画等待帧不会被当成摸牌，补牌到达后才确认新摸入牌", () => {
    const currentPlayer = tablePreviewView.players.find(
      (player) => player.seat === 3,
    )!;
    const previousPlayer = {
      ...currentPlayer,
      melds: [
        {
          ...currentPlayer.melds[0]!,
          kind: "pon" as const,
          tiles: currentPlayer.melds[0]!.tiles.slice(0, 3),
        },
        currentPlayer.melds[1]!,
      ],
    };
    const beforeKanView: MatchView = {
      ...tablePreviewView,
      variant_kind: "impact",
      phase: { kind: "awaiting_turn_action", seat: currentPlayer.seat },
      players: tablePreviewView.players.map((player) =>
        player.seat === currentPlayer.seat ? previousPlayer : player,
      ),
    };
    const waitingView: MatchView = {
      ...tablePreviewView,
      variant_kind: "impact",
      completed_rinshan_draws: countCompletedKans(tablePreviewView) - 1,
      phase: {
        kind: "awaiting_kan_animation",
        seat: currentPlayer.seat,
      },
      players: tablePreviewView.players.map((player) =>
        player.seat === currentPlayer.seat ? currentPlayer : player,
      ),
    };
    const drawnView: MatchView = {
      ...waitingView,
      completed_rinshan_draws: waitingView.completed_rinshan_draws! + 1,
      remaining_live_draws: waitingView.remaining_live_draws - 1,
      phase: { kind: "awaiting_turn_action", seat: currentPlayer.seat },
      players: tablePreviewView.players,
    };

    expect(completedImpactRinshanDraws(waitingView)).toBe(
      countCompletedKans(waitingView) - 1,
    );
    expect(completedImpactRinshanDraws(drawnView)).toBe(
      countCompletedKans(drawnView),
    );

    expect(
      playerReceivedDraw(
        waitingView,
        beforeKanView,
        currentPlayer,
        previousPlayer,
      ),
    ).toBe(false);
    expect(playerCompletedKan(currentPlayer, previousPlayer)).toBe(true);
    expect(playerCompletedKan(currentPlayer, currentPlayer)).toBe(false);
    expect(
      playerReceivedDraw(
        drawnView,
        waitingView,
        currentPlayer,
        currentPlayer,
      ),
    ).toBe(true);
    expect(resolveRinshanDrawNumber(true, 2, 2, 1)).toBe(2);
    expect(
      resolveRinshanDrawNumber(
        true,
        undefined,
        drawnView.completed_rinshan_draws!,
        waitingView.completed_rinshan_draws!,
      ),
    ).toBe(drawnView.completed_rinshan_draws);
    expect(resolveRinshanDrawNumber(true, undefined, 2, 2)).toBeNull();
  });

  it("别家拔北后的补牌也会被识别成共享岭上序列的一次新摸牌", () => {
    const basePlayer = tablePreviewView.players.find(
      (player) => player.seat !== tablePreviewView.observer_seat,
    )!;
    const previousPlayer = {
      ...basePlayer,
      nuki_tiles: [],
    };
    const currentPlayer = {
      ...basePlayer,
      nuki_tiles: [{ id: 9000, code: "4z" }],
    };
    const previousView: MatchView = {
      ...tablePreviewView,
      phase: { kind: "awaiting_turn_action", seat: currentPlayer.seat },
      players: tablePreviewView.players.map((player) =>
        player.seat === currentPlayer.seat ? previousPlayer : player,
      ),
    };
    const currentView: MatchView = {
      ...previousView,
      players: previousView.players.map((player) =>
        player.seat === currentPlayer.seat ? currentPlayer : player,
      ),
    };

    expect(playerExtractedNorth(currentPlayer, previousPlayer)).toBe(true);
    expect(
      playerReceivedDraw(
        currentView,
        previousView,
        currentPlayer,
        previousPlayer,
      ),
    ).toBe(true);
  });

  it("加杠先落牌时不误报摸牌，岭上计数增加后才识别补摸", () => {
    const placedPlayer = tablePreviewView.players.find(
      (player) => player.seat !== tablePreviewView.observer_seat,
    )!;
    const ponPlayer = {
      ...placedPlayer,
      melds: placedPlayer.melds.map((meld, index) =>
        index === 0
          ? { ...meld, kind: "pon" as const, tiles: meld.tiles.slice(0, 3) }
          : meld,
      ),
    };
    const beforeView: MatchView = {
      ...tablePreviewView,
      completed_rinshan_draws: 0,
      phase: { kind: "awaiting_turn_action", seat: placedPlayer.seat },
      players: tablePreviewView.players.map((player) =>
        player.seat === placedPlayer.seat ? ponPlayer : player,
      ),
    };
    const responseView: MatchView = {
      ...beforeView,
      phase: { kind: "awaiting_responses", trigger_seat: placedPlayer.seat },
      players: beforeView.players.map((player) =>
        player.seat === placedPlayer.seat ? placedPlayer : player,
      ),
    };
    const drawnPlayer = {
      ...placedPlayer,
      concealed_tile_count: placedPlayer.concealed_tile_count + 1,
    };
    const drawnView: MatchView = {
      ...responseView,
      completed_rinshan_draws: 1,
      remaining_live_draws: responseView.remaining_live_draws - 1,
      phase: { kind: "awaiting_turn_action", seat: placedPlayer.seat },
      players: responseView.players.map((player) =>
        player.seat === placedPlayer.seat ? drawnPlayer : player,
      ),
    };

    expect(
      playerReceivedDraw(responseView, beforeView, placedPlayer, ponPlayer),
    ).toBe(false);
    expect(
      playerReceivedDraw(drawnView, responseView, drawnPlayer, placedPlayer),
    ).toBe(true);
  });
});

describe("四川庄家开局第十四张", () => {
  it("开局已有的第十四张不算普通摸牌，之后的摸牌仍正常播放", () => {
    const players = tablePreviewView.players.map((player) => ({
      ...player,
      melds: [],
      discards: [],
    }));
    const dealer = players[0]!;
    dealer.concealed_tile_count = 14;
    dealer.drawn_tile_id = dealer.concealed_tiles?.at(-1)?.id ?? null;
    const openingView: MatchView = {
      ...tablePreviewView,
      variant_kind: "sichuan",
      completed_rinshan_draws: 0,
      progress: { ...tablePreviewView.progress, dealer: dealer.seat },
      players,
    };

    expect(isSichuanOpeningDealerDraw(openingView, dealer)).toBe(true);

    const afterFirstDiscard = structuredClone(openingView);
    afterFirstDiscard.players[0]!.discards.push({
      tile: { id: 9900, code: "1m" },
      tsumogiri: false,
      riichi_declared: false,
      claimed_by: null,
    });
    expect(
      isSichuanOpeningDealerDraw(
        afterFirstDiscard,
        afterFirstDiscard.players[0]!,
      ),
    ).toBe(false);

    const rinshanView = structuredClone(openingView);
    rinshanView.completed_rinshan_draws = 1;
    expect(
      isSichuanOpeningDealerDraw(rinshanView, rinshanView.players[0]!),
    ).toBe(false);
  });
});

describe("正方形牌桌对称布局", () => {
  it("桌上牌宽长比会同步改变牌河、手牌与牌山横向步距", () => {
    const widthRatio = 0.82;
    const expectedWidth = 0.56 * widthRatio;
    expect(
      discardGridPosition(1, widthRatio, 1).x -
        discardGridPosition(0, widthRatio, 1).x,
    ).toBeCloseTo(expectedWidth + 0.03);
    expect(
      handPosition(0, 13, 1, false, 0, false, widthRatio, 1).x -
        handPosition(0, 13, 0, false, 0, false, widthRatio, 1).x,
    ).toBeCloseTo(expectedWidth + HAND_TILE_GAP);
    /* 摸牌序号顺着摸牌方向走，也就是往左，所以步距是负的。 */
    expect(
      wallTileOrigin(2, widthRatio, 1).x -
        wallTileOrigin(0, widthRatio, 1).x,
    ).toBeCloseTo(-expectedWidth);
  });

  it("四家手牌使用同一局部坐标旋转，距桌边余量完全一致", () => {
    const zones = tableLayoutZones();
    expect(zones.table.imageHalfSide).toBe(8.1);
    expect(zones.table.clothHalfSide).toBe(6);
    const positions = [0, 1, 2, 3].map((relative) =>
      handPosition(relative, 13, 0, relative === 0),
    );
    const radii = positions.map((position) =>
      Math.hypot(position.x, position.z),
    );
    radii.forEach((radius) => expect(radius).toBeCloseTo(radii[0]!));
    positions.forEach((position) => {
      expect(Math.abs(position.x)).toBeLessThan(5.63);
      expect(Math.abs(position.z)).toBeLessThan(5.63);
    });
  });

  it("宽屏取景让桌布覆盖窗口而不是缩在中央", () => {
    const aspect = 16 / 9;
    const layout = tableCameraLayout(aspect);
    const camera = new THREE.PerspectiveCamera(layout.fov, aspect, 0.1, 180);
    camera.position.set(0, layout.y, layout.z);
    camera.lookAt(0, layout.targetY, layout.targetZ);
    camera.updateMatrixWorld();
    camera.updateProjectionMatrix();
    const far = new THREE.Vector3(6, 0, -6).project(camera);
    const near = new THREE.Vector3(6, 0, 6).project(camera);
    const selfMeld = meldTilePosition(0, 0, false).project(camera);

    expect(layout.y).toBe(21);
    expect(layout.z).toBe(18.2);
    const tableAngle =
      THREE.MathUtils.radToDeg(
        Math.atan2(
          layout.y - layout.targetY,
          layout.z - layout.targetZ,
        ),
      );
    expect(tableAngle).toBeCloseTo(50, 1);
    expect(far.x).toBeGreaterThan(0.58);
    expect(far.y).toBeGreaterThan(0.88);
    expect(near.x).toBeGreaterThan(0.75);
    expect(near.y).toBeLessThan(-0.85);
    expect(selfMeld.y).toBeGreaterThan(-0.9);
  });

  it("正交摄像机按宽高比保持对称取景边界", () => {
    expect(orthographicCameraBounds(12, 16 / 9)).toEqual({
      left: -(32 / 3),
      right: 32 / 3,
      top: 6,
      bottom: -6,
    });
  });
});

describe("倒牌方向", () => {
  const faceNormal = (tilt: number, backMesh: boolean) => {
    const axis = backMesh ? -1 : 1;
    return {
      y: Number((axis * Math.cos(tilt)).toFixed(6)),
      z: Number((axis * Math.sin(tilt)).toFixed(6)),
    };
  };

  it.each([false, true])(
    "立牌时白面朝自己、绿背朝牌桌中央（背面牌=%s）",
    (backMesh) => {
      const normal = faceNormal(standingHandTilt(backMesh), backMesh);
      expect(normal.z).toBeGreaterThan(0.99);
      expect(normal.y).toBeGreaterThan(0);
      expect(normal.y).toBeLessThan(0.2);
    },
  );

  it.each([false, true])("摊牌后白面朝上（背面牌=%s）", (backMesh) => {
    const normal = faceNormal(settlementHandTilt(true, backMesh), backMesh);
    expect(normal.y).toBeCloseTo(1);
    expect(normal.z).toBeCloseTo(0);
  });

  it.each([false, true])("盖牌后白面朝下（背面牌=%s）", (backMesh) => {
    const normal = faceNormal(settlementHandTilt(false, backMesh), backMesh);
    expect(normal.y).toBeCloseTo(-1);
    expect(normal.z).toBeCloseTo(0);
  });

  it("摊牌沿外侧边向内倒，盖牌沿内侧边向外倒", () => {
    for (const backMesh of [false, true]) {
      const stand = standingHandTilt(backMesh);
      // 摊牌 keeps turning in one direction, 盖牌 in the other.
      expect(settlementHandTilt(true, backMesh)).toBeLessThan(stand);
      expect(settlementHandTilt(false, backMesh)).toBeGreaterThan(stand);
    }
    expect(settlementHandShift(true, 0.54, 0.27)).toBeCloseTo(-0.135);
    expect(settlementHandShift(false, 0.54, 0.27)).toBeCloseTo(0.135);
  });
});

describe("结算盖牌座次", () => {
  const view = (
    reason: "ron" | "tsumo",
    winnerSeat: number,
    fromSeat: number | null,
  ) =>
    ({
      players: [0, 1, 2, 3].map((seat) => ({ seat })),
      hand_settlement: {
        reason,
        from_seat: fromSeat,
        winners: [{ seat: winnerSeat }],
      },
    }) as unknown as MatchView;

  it("荣和只有点炮者盖牌", () => {
    expect(settlementCoveringSeats(view("ron", 0, 2))).toEqual([2]);
  });

  it("自摸其余所有人盖牌", () => {
    expect(settlementCoveringSeats(view("tsumo", 1, null))).toEqual([0, 2, 3]);
  });
});

describe("三麻固定座位", () => {
  it("东家视角固定空出左手北位", () => {
    expect(tableRelativeSeat(0, 0, 3)).toBe(0);
    expect(tableRelativeSeat(1, 0, 3)).toBe(1);
    expect(tableRelativeSeat(2, 0, 3)).toBe(2);
    expect([0, 1, 2].map((seat) => tableRelativeSeat(seat, 0, 3))).not.toContain(
      3,
    );
  });

  it("其他玩家视角仍沿固定四方位旋转", () => {
    expect(tableRelativeSeat(0, 1, 3)).toBe(3);
    expect(tableRelativeSeat(1, 1, 3)).toBe(0);
    expect(tableRelativeSeat(2, 1, 3)).toBe(1);
  });
});

describe("三麻牌山", () => {
  it("只在三名玩家面前各摆18墩，空北位没有牌", () => {
    const layout = sanmaWallLayout(0, 0, [2, 3]);
    const slots = Array.from({ length: layout.drawableCount }, (_, order) =>
      layout.drawSlot(order),
    );

    expect(layout.drawableCount).toBe(108);
    expect(new Set(slots).size).toBe(108);
    expect(new Set(slots.map((slot) => Math.floor(slot / 36)))).toEqual(
      new Set([0, 1, 2]),
    );
    for (const side of [0, 1, 2]) {
      expect(slots.filter((slot) => Math.floor(slot / 36) === side)).toHaveLength(36);
    }
  });

  it("换视角后仍空出绝对北位，而不是固定空屏幕左侧", () => {
    const layout = sanmaWallLayout(0, 1, [1, 1]);
    const sides = new Set(
      Array.from({ length: 108 }, (_, order) =>
        Math.floor(layout.drawSlot(order) / 36),
      ),
    );

    expect(sides).toEqual(new Set([0, 1, 3]));
    expect(sides.has(2)).toBe(false);
  });

  it("骰子从庄家算1并在三家之间循环，直接跳过空北位", () => {
    // 绝对西家(2)掷出2点，应数到绝对东家(0)，不会落在空着的北位(3)。
    const layout = sanmaWallLayout(2, 0, [1, 1]);
    expect(Math.floor(layout.drawSlot(0) / 36)).toBe(0);
  });

  it("拔北与役牌模式分别从第9张和第5张位置开始翻宝牌", () => {
    const nuki = sanmaWallLayout(0, 0, [2, 3], "nuki_dora");
    const yakuhai = sanmaWallLayout(0, 0, [2, 3], "yakuhai");

    expect(Array.from({ length: 5 }, (_, i) => nuki.doraOrderIndex(i))).toEqual([
      98, 100, 102, 104, 106,
    ]);
    expect(Array.from({ length: 5 }, (_, i) => yakuhai.doraOrderIndex(i))).toEqual([
      102, 100, 98, 96, 94,
    ]);
    const sharedRinshanOrder = [106, 107, 104, 105];
    expect(Array.from({ length: 4 }, (_, i) => nuki.rinshanOrderIndex(i + 1))).toEqual(
      sharedRinshanOrder,
    );
    expect(Array.from({ length: 4 }, (_, i) => yakuhai.rinshanOrderIndex(i + 1))).toEqual(
      sharedRinshanOrder,
    );
  });
});

describe("点牌舍牌阶段", () => {
  it.each(["awaiting_turn_action", "awaiting_discard"] as const)(
    "%s 阶段允许当前玩家直接点牌",
    (kind) => {
      const view = {
        observer_seat: 1,
        phase: { kind, seat: 1 },
      } as MatchView;
      expect(canLocalPlayerDiscard(view)).toBe(true);
    },
  );
});

describe("摸牌间隔", () => {
  it.each(["awaiting_turn_action", "awaiting_discard"] as const)(
    "%s 阶段为当前座位保留摸牌位置",
    (kind) => {
      const view = {
        phase: { kind, seat: 2 },
      } as MatchView;

      expect(playerIsHoldingDrawnTile(view, 2)).toBe(true);
      expect(playerIsHoldingDrawnTile(view, 1)).toBe(false);
    },
  );

  it("舍牌响应阶段不再保留摸牌位置", () => {
    const view = {
      phase: { kind: "awaiting_responses", trigger_seat: 2 },
    } as MatchView;

    expect(playerIsHoldingDrawnTile(view, 2)).toBe(false);
  });

  it("对手摸牌后保留全部十四张，并把摸入牌单列在末端", () => {
    const layout = opponentHandLayout(14, true);

    expect(layout.renderedSlots).toHaveLength(14);
    expect(layout.renderedSlots).toEqual(
      Array.from({ length: 14 }, (_, index) => index),
    );
    expect(layout.drawnSlot).toBe(13);
  });

  it("摸切后直接回到十三张紧凑牌阵", () => {
    const layout = opponentHandLayout(13, false);

    expect(layout.slotCount).toBe(13);
    expect(layout.renderedSlots).toHaveLength(13);
    expect(layout.drawnSlot).toBeNull();
  });

  it("手切后十三张牌保持不变，只在基础牌阵留下一个空槽", () => {
    const layout = opponentHandLayout(13, false, 5);

    expect(layout.slotCount).toBe(14);
    expect(layout.renderedSlots).toHaveLength(13);
    expect(layout.renderedSlots).not.toContain(5);
    expect(layout.renderedSlots.at(-1)).toBe(13);
    expect(layout.drawnSlot).toBe(13);
  });
});

describe("四川自摸盖牌", () => {
  it("摸牌 id 隐藏后仍按胡牌记录识别自摸牌", () => {
    const view = structuredClone(tablePreviewView);
    view.variant_kind = "sichuan";
    const player = view.players[0]!;
    player.won = true;
    player.winning_tile = { id: 9001, code: "5p" };
    player.drawn_tile_id = null;
    player.win_is_tsumo = true;

    expect(playerSichuanWinIsTsumo(view, player)).toBe(true);
  });
});

describe("开局取牌顺序", () => {
  it("庄家从骰子确定的断点先取四枚，各家依序取牌", () => {
    expect(openingDealOrder(0, 2, 2, 4)).toBe(0);
    expect(openingDealOrder(3, 2, 2, 4)).toBe(3);
    expect(openingDealOrder(0, 3, 2, 4)).toBe(4);
    expect(openingDealOrder(4, 2, 2, 4)).toBe(16);
  });

  it("取完三轮四枚后依序补第十三枚，庄家再取第十四枚", () => {
    expect(openingDealOrder(12, 2, 2, 4)).toBe(48);
    expect(openingDealOrder(12, 1, 2, 4)).toBe(51);
    expect(openingDealOrder(13, 2, 2, 4)).toBe(52);
  });

  it("同家一次取出的四枚同批移动，不同座次严格依序", () => {
    expect(openingDealStep(0, 2, 2, 4)).toBe(0);
    expect(openingDealStep(3, 2, 2, 4)).toBe(0);
    expect(openingDealStep(0, 3, 2, 4)).toBe(1);
    expect(openingDealStep(4, 2, 2, 4)).toBe(4);
    expect(openingDealStep(12, 2, 2, 4)).toBe(12);
    expect(openingDealStep(13, 2, 2, 4)).toBe(16);
    expect(openingDealArrival(0, 2, 2, 4)).toBe(180);
    expect(openingDealArrival(0, 3, 2, 4)).toBe(300);
    expect(openingDealDuration(4)).toBeGreaterThan(2_000);
  });

  it("牌山里的牌是盖着平躺的，落到手上才立起来", () => {
    for (const backMesh of [false, true]) {
      /* 平躺盖着 = 结算时被扣下去的那个角度，和立着完全不是一回事。 */
      expect(coveredHandTilt(backMesh)).toBe(settlementHandTilt(false, backMesh));
      expect(coveredHandTilt(backMesh)).not.toBe(standingHandTilt(backMesh));
    }
  });

  it("翻牌是单向减速，不会回弹穿过桌面", () => {
    expect(standUpEase(0)).toBe(0);
    expect(standUpEase(1)).toBe(1);
    for (let step = 1; step <= 10; step += 1) {
      const previous = standUpEase((step - 1) / 10);
      const current = standUpEase(step / 10);
      expect(current).toBeGreaterThan(previous);
      expect(current).toBeLessThanOrEqual(1);
    }
  });

  it("整段开局动画要等最后一张牌翻起来立住", () => {
    expect(openingDealDuration(4)).toBeGreaterThanOrEqual(
      16 * 120 + 180 + TILE_STAND_UP_MS,
    );
  });
});

describe("牌河采用牌", () => {
  it("从牌河移除已被副露采用的弃牌并保留原索引", () => {
    const entries = riverDiscardEntries([
      {
        tile: { id: 1, code: "1m" },
        tsumogiri: false,
        riichi_declared: false,
        claimed_by: null,
      },
      {
        tile: { id: 2, code: "2m" },
        tsumogiri: false,
        riichi_declared: false,
        claimed_by: 1,
      },
      {
        tile: { id: 3, code: "3m" },
        tsumogiri: true,
        riichi_declared: false,
        claimed_by: null,
      },
    ]);

    expect(entries.map(({ discard }) => discard.tile.id)).toEqual([1, 3]);
    expect(entries.map(({ originalIndex }) => originalIndex)).toEqual([0, 2]);
  });

  it("冲击麻将只记了 claimed，一样要从牌河里拿走", () => {
    /* 冲击麻将记不到是被谁鸣的，`claimed_by` 恒为 null，只有 `claimed`。 */
    const entries = riverDiscardEntries([
      {
        tile: { id: 1, code: "1m" },
        tsumogiri: false,
        riichi_declared: false,
        claimed_by: null,
        claimed: false,
      },
      {
        tile: { id: 2, code: "2m" },
        tsumogiri: false,
        riichi_declared: false,
        claimed_by: null,
        claimed: true,
      },
      {
        tile: { id: 3, code: "3m" },
        tsumogiri: true,
        riichi_declared: false,
        claimed_by: null,
        claimed: false,
      },
    ]);

    expect(entries.map(({ discard }) => discard.tile.id)).toEqual([1, 3]);
    expect(entries.map(({ originalIndex }) => originalIndex)).toEqual([0, 2]);
  });

  it("横置立直宣言牌", () => {
    const entries = riverDiscardEntries([
      river(1, false, null),
      river(2, true, null),
      river(3, false, null),
    ]);

    expect(entries.filter(({ sideways }) => sideways).map(({ discard }) =>
      discard.tile.id,
    )).toEqual([2]);
  });

  it("宣言牌被鸣走之后横置下一张仍在牌河里的牌", () => {
    const entries = riverDiscardEntries([
      river(1, false, null),
      river(2, true, 1),
      river(3, false, null),
      river(4, false, null),
    ]);

    expect(entries.filter(({ sideways }) => sideways).map(({ discard }) =>
      discard.tile.id,
    )).toEqual([3]);
  });

  it("补上的那张又被鸣走就继续往后顺，一张不横也不行", () => {
    const entries = riverDiscardEntries([
      river(1, true, 2),
      river(2, false, 3),
      river(3, false, null),
    ]);

    expect(entries.filter(({ sideways }) => sideways).map(({ discard }) =>
      discard.tile.id,
    )).toEqual([3]);
  });

  it("没人立直就没有横置的牌", () => {
    const entries = riverDiscardEntries([
      river(1, false, null),
      river(2, false, null),
    ]);

    expect(entries.every(({ sideways }) => !sideways)).toBe(true);
  });
});

function river(
  id: number,
  riichiDeclared: boolean,
  claimedBy: number | null,
): DiscardView {
  return {
    tile: { id, code: "1m" },
    tsumogiri: false,
    riichi_declared: riichiDeclared,
    claimed_by: claimedBy,
  };
}

describe("宝牌判定", () => {
  it("四麻翻倒数第三摞上层牌，三麻翻倒数第五摞上层牌", () => {
    expect(doraWallTileIndex(83, 4)).toBe(77);
    expect(doraWallTileIndex(68, 3)).toBe(58);
    expect(136 - 83 + doraWallTileIndex(83, 4)).toBe(130);
    expect(136 - 68 + doraWallTileIndex(68, 3)).toBe(126);
  });

  it("杠宝牌沿各自规则保持翻开上层牌", () => {
    expect(
      Array.from({ length: 5 }, (_, index) =>
        doraWallTileIndex(83, 4, index),
      ),
    ).toEqual([77, 75, 73, 71, 69]);
    expect(
      Array.from({ length: 5 }, (_, index) =>
        doraWallTileIndex(68, 3, index),
      ),
    ).toEqual([58, 60, 62, 64, 66]);
  });

  it("活牌减少后宝牌指示牌仍固定在同一实体牌摞", () => {
    expect(136 - 83 + doraWallTileIndex(83, 4)).toBe(
      136 - 57 + doraWallTileIndex(57, 4),
    );
    expect(136 - 68 + doraWallTileIndex(68, 3)).toBe(
      136 - 42 + doraWallTileIndex(42, 3),
    );
  });

  it("岭上牌空位不会推动宝牌指示牌位置", () => {
    expect(136 - 81 - 2 + doraWallTileIndex(81, 4, 0, 2)).toBe(130);
    expect(136 - 66 - 2 + doraWallTileIndex(66, 3, 0, 2)).toBe(126);
  });

  it("处理数牌、风牌和三元牌循环", () => {
    expect(doraCodeForIndicator("9m")).toBe("1m");
    expect(doraCodeForIndicator("4z")).toBe("1z");
    expect(doraCodeForIndicator("7z")).toBe("5z");
  });

  it("红五与普通五都按五计算", () => {
    expect(doraCodeForIndicator("0p")).toBe("6p");
    expect(isDoraTile("0s", [{ code: "4s" }])).toBe(true);
    expect(isDoraTile("5s", [{ code: "4s" }])).toBe(true);
  });

  it("赤宝牌无需宝牌指示牌也会高亮", () => {
    expect(isDoraTile("0m", [])).toBe(true);
    expect(isDoraTile("5m", [])).toBe(false);
  });
});

describe("牌河排列", () => {
  it("每排从同一左端开始并且为自然偏角预留间隙", () => {
    expect(discardGridPosition(0).x).toBe(discardGridPosition(6).x);
    expect(discardGridPosition(6).x).toBe(discardGridPosition(12).x);
    expect(discardGridPosition(1).x - discardGridPosition(0).x).toBeCloseTo(
      0.417072,
    );
    expect(discardGridPosition(6).z - discardGridPosition(0).z).toBeCloseTo(
      0.5676,
    );
  });

  it("立直三麻和四麻都保留第四行，拔北从第三行第八格起向右排列", () => {
    expect(discardGridPosition(18).z).toBeGreaterThan(discardGridPosition(12).z);
    expect(discardGridPosition(24).z).toBe(discardGridPosition(18).z);
    const thirdRowStart = discardGridPosition(12);
    const columnStep = discardGridPosition(13).x - thirdRowStart.x;
    expect(nukiRiverPosition(0).z).toBe(thirdRowStart.z);
    expect(nukiRiverPosition(0).x).toBeCloseTo(thirdRowStart.x + 7 * columnStep);
    expect(nukiRiverPosition(1).x).toBeGreaterThan(nukiRiverPosition(0).x);
    expect(nukiRiverPosition(1).z).toBe(nukiRiverPosition(0).z);
  });

  it("牌河偏角细微且对同一张牌保持稳定", () => {
    const first = discardNaturalRotation(2, 7);
    expect(discardNaturalRotation(2, 7)).toBe(first);
    expect(discardNaturalRotation(2, 8)).not.toBe(first);
    const angles = [0, 1, 2, 3].flatMap((seat) =>
      Array.from({ length: 51 }, (_, index) =>
        Math.abs(
          THREE.MathUtils.radToDeg(
            discardNaturalRotation(seat, index),
          ),
        ),
      ),
    );
    const largeAngles = angles.filter((angle) => angle >= 4);
    expect(Math.max(...angles)).toBeLessThanOrEqual(5.5);
    expect(largeAngles.length).toBeGreaterThan(0);
    expect(largeAngles.length / angles.length).toBeLessThan(0.1);
  });
});

describe("四川换三张落桌位置", () => {
  it("顺逆时针沿四家整圈传递，对家才两两交换", () => {
    expect(
      [0, 1, 2, 3].map((seat) =>
        exchangeRecipient(seat, "counter_clockwise"),
      ),
    ).toEqual([1, 2, 3, 0]);
    expect(
      [0, 1, 2, 3].map((seat) => exchangeRecipient(seat, "clockwise")),
    ).toEqual([3, 0, 1, 2]);
    expect(
      [0, 1, 2, 3].map((seat) => exchangeRecipient(seat, "opposite")),
    ).toEqual([2, 3, 0, 1]);
  });

  it("落在牌河外缘与牌山内缘之间，不占用牌河行", () => {
    const position = exchangeStackPosition(0, 1);
    const riverOuterEdge =
      discardGridPosition(12).z + RIVER_TILE_LENGTH * 0.96 / 2;
    const wallInnerEdge = WALL_DISTANCE - WALL_TILE_LENGTH * 0.96 / 2;

    expect(position.z).toBeGreaterThan(riverOuterEdge);
    expect(position.z).toBeLessThan(wallInnerEdge);
    expect(position.z).toBeCloseTo((riverOuterEdge + wallInnerEdge) / 2);
  });

  it("四家只旋转同一个共享坐标，不分别添加 z 偏移", () => {
    const self = exchangeStackPosition(0, 0);
    const right = exchangeStackPosition(1, 0);
    const opposite = exchangeStackPosition(2, 0);
    const left = exchangeStackPosition(3, 0);

    expect(right.x).toBeCloseTo(self.z);
    expect(right.z).toBeCloseTo(-self.x);
    expect(opposite.x).toBeCloseTo(-self.x);
    expect(opposite.z).toBeCloseTo(-self.z);
    expect(left.x).toBeCloseTo(-self.z);
    expect(left.z).toBeCloseTo(self.x);
  });
});

describe("倒牌动画曲线", () => {
  it("从静止开始并停在终点", () => {
    expect(settlementFallEase(0)).toBe(0);
    expect(settlementFallEase(1)).toBeCloseTo(1, 5);
  });

  it("落地前始终加速前进", () => {
    let previous = -1;
    for (let step = 0; step <= 62; step += 1) {
      const value = settlementFallEase(step / 100);
      expect(value).toBeGreaterThan(previous);
      previous = value;
    }
    // 前半段慢、后半段快，才有砸下去的力量感
    expect(settlementFallEase(0.31)).toBeLessThan(settlementFallEase(0.62) / 2);
  });

  it("回弹不会把牌翻过桌面", () => {
    for (let step = 0; step <= 100; step += 1) {
      const value = settlementFallEase(step / 100);
      expect(value).toBeLessThanOrEqual(1);
      expect(value).toBeGreaterThanOrEqual(0);
    }
    // 落地后有明显回弹
    expect(settlementFallEase(0.7)).toBeLessThan(1);
  });

  it("超出范围的进度被夹紧", () => {
    expect(settlementFallEase(-1)).toBe(0);
    expect(settlementFallEase(2)).toBeCloseTo(1, 5);
  });
});

describe("自摸牌定位", () => {
  const tiles = [{ id: 3 }, { id: 7 }, { id: 11 }];

  it("优先使用摸到的那张牌", () => {
    expect(winningTileIndex(tiles, 7)).toBe(1);
  });

  it("没有摸牌信息时取最后一张", () => {
    expect(winningTileIndex(tiles, null)).toBe(2);
    expect(winningTileIndex(tiles, 99)).toBe(2);
  });
});

describe("主视角摸牌落到二维手牌那一格", () => {
  /** 默认那台相机，和 `tableCameraLayout` 给的一样。 */
  function tableCamera(width: number, height: number) {
    const layout = tableCameraLayout(width / height);
    const camera = new THREE.PerspectiveCamera(
      layout.fov,
      width / height,
      0.1,
      600,
    );
    camera.position.set(0, layout.y, layout.z);
    camera.lookAt(0, layout.targetY, layout.targetZ);
    camera.updateMatrixWorld(true);
    camera.updateProjectionMatrix();
    return camera;
  }

  const canvas = { width: 1600, height: 900 };
  const camera = tableCamera(canvas.width, canvas.height);
  const baseline = handPosition(0, 14, 13, true, 0.2);

  it("算出来的落点投影回屏幕，正好落在那一格上", () => {
    const rect = { centerX: 1180, centerY: 820, width: 61.5 };
    const anchor = screenRectAnchor(
      camera,
      canvas,
      rect,
      baseline,
      TILE_LENGTH * TILE_WIDTH_RATIO,
    );
    const projected = anchor.position.clone().project(camera);
    expect(((projected.x + 1) / 2) * canvas.width).toBeCloseTo(
      rect.centerX,
      3,
    );
    expect(((1 - projected.y) / 2) * canvas.height).toBeCloseTo(
      rect.centerY,
      3,
    );
  });

  it("二维手牌比三维那条基线大，所以要放大着落地", () => {
    const anchor = screenRectAnchor(
      camera,
      canvas,
      { centerX: 1180, centerY: 820, width: 61.5 },
      baseline,
      TILE_LENGTH * TILE_WIDTH_RATIO,
    );
    // 牌桌上的牌按 DEFAULT_TILE_SCALE 摆，落到手牌那一格得再涨一截
    expect(anchor.scale).toBeGreaterThan(1);
  });

  it("那一格越宽，牌就得摆得越大", () => {
    const narrow = screenRectAnchor(
      camera,
      canvas,
      { centerX: 1180, centerY: 820, width: 40 },
      baseline,
      TILE_LENGTH * TILE_WIDTH_RATIO,
    );
    const wide = screenRectAnchor(
      camera,
      canvas,
      { centerX: 1180, centerY: 820, width: 80 },
      baseline,
      TILE_LENGTH * TILE_WIDTH_RATIO,
    );
    expect(wide.scale / narrow.scale).toBeCloseTo(2, 6);
  });

  it("落点和基线在同一层等深面上，只挪横竖不改深浅", () => {
    const anchor = screenRectAnchor(
      camera,
      canvas,
      { centerX: 1180, centerY: 820, width: 61.5 },
      baseline,
      TILE_LENGTH * TILE_WIDTH_RATIO,
    );
    const depthOf = (point: THREE.Vector3) =>
      camera.worldToLocal(point.clone()).z;
    expect(depthOf(anchor.position)).toBeCloseTo(depthOf(baseline), 4);
  });

  it("立起来的角度让牌面正对镜头，比桌上那个立牌角度更平", () => {
    const tilt = billboardHandTilt(camera);
    // 牌面法线 (0, cos, sin) 应当正好是取景方向的反向
    const forward = new THREE.Vector3(0, 0, -1).applyQuaternion(
      camera.quaternion,
    );
    expect(Math.cos(tilt)).toBeCloseTo(-forward.y, 6);
    expect(Math.sin(tilt)).toBeCloseTo(-forward.z, 6);
    // 从盖着（π）往这个角度转，方向和桌上立牌是同一个，只是转得更过去一点
    expect(tilt).toBeLessThan(standingHandTilt(false));
    expect(tilt).toBeGreaterThan(0);
  });
});

describe("副露推牌", () => {
  it("起点在副露位靠手牌的一侧", () => {
    const destination = new THREE.Vector3(4.2, 0.22, 5.35);
    const start = meldPushSource(destination, 0);

    expect(start.x).toBeLessThan(destination.x);
    // 推的过程贴着桌面，不抬高
    expect(start.y).toBeCloseTo(destination.y, 6);
    // 起点比副露位更靠近自己
    expect(start.z).toBeGreaterThan(destination.z);
  });

  it("对家的推牌方向跟着座位转", () => {
    const destination = new THREE.Vector3(-4.2, 0.22, -5.35);
    const start = meldPushSource(destination, 2);

    expect(start.x).toBeGreaterThan(destination.x);
    expect(start.z).toBeLessThan(destination.z);
    expect(start.distanceTo(destination)).toBeCloseTo(
      meldPushSource(new THREE.Vector3(4.2, 0.22, 5.35), 0).distanceTo(
        new THREE.Vector3(4.2, 0.22, 5.35),
      ),
      6,
    );
  });

  it("推牌时长是一个正数", () => {
    expect(MELD_PUSH_MS).toBeGreaterThan(0);
  });
});

describe("自摸甩牌", () => {
  it("开头就在最高处，一路掉到桌面", () => {
    // 出手那一下就是满高度，不是先抬起来再落
    expect(tsumoThrowArc(0)).toBeCloseTo(1, 6);
    // 一路往下，没有回头
    let previous = 1;
    for (const progress of [0.2, 0.4, 0.6, 0.8, 1]) {
      const height = tsumoThrowArc(progress);
      expect(height).toBeLessThan(previous);
      previous = height;
    }
    // 顶上掉得慢、贴桌面掉得快
    expect(tsumoThrowArc(0.5)).toBeGreaterThan(0.5);
    // 落地贴着桌面
    expect(tsumoThrowArc(1)).toBeCloseTo(0, 6);
  });

  it("落地就停，不再弹起来", () => {
    // 整段之内高度单调下降，落地之后一直贴着桌面
    expect(tsumoThrowArc(0.9)).toBeGreaterThan(tsumoThrowArc(0.95));
    expect(tsumoThrowArc(1)).toBeCloseTo(0, 6);
    expect(tsumoThrowArc(1.4)).toBeCloseTo(0, 6);
  });

  it("位移和摊平跟着下落走，落地那一刻正好到位", () => {
    expect(tsumoThrowEase(0)).toBeCloseTo(0, 6);
    expect(tsumoThrowEase(1)).toBeCloseTo(1, 6);
    expect(tsumoThrowEase(1.4)).toBeCloseTo(1, 6);
    // 顶上还立着，贴桌面那一下才甩平
    expect(tsumoThrowEase(0.5)).toBeLessThan(0.5);
  });

  it("扬灰排在牌落地的那一刻，散完就拆掉", () => {
    const impact = {
      mesh: {
        visible: false,
        scale: { setScalar: () => {} },
        geometry: { dispose: () => {} },
        removeFromParent: () => {},
      },
      material: { opacity: 0, dispose: () => {} },
      startedAt: 1000,
      duration: 400,
    } as unknown as TableImpact;
    // 牌还在半空，这层灰先藏着
    expect(advanceTableImpacts([impact], 900)).toHaveLength(1);
    expect(impact.mesh.visible).toBe(false);
    // 落地之后按进度扬起来
    expect(advanceTableImpacts([impact], 1200)).toHaveLength(1);
    expect(impact.mesh.visible).toBe(true);
    expect(impact.material.opacity).toBeGreaterThan(0);
    expect(impact.material.opacity).toBeLessThan(0.62);
    // 散完就不留在场景里
    expect(advanceTableImpacts([impact], 1400)).toHaveLength(0);
  });

  it("镜头砸下去就颤到最大，之后迅速收住", () => {
    const start = cameraShakeOffset(0);
    // 被撞的那一瞬就偏出去，不是慢慢晃起来
    expect(Math.abs(start.y)).toBeCloseTo(1, 6);
    // 越往后越小
    expect(Math.abs(cameraShakeOffset(0.5).y)).toBeLessThan(0.5);
    expect(cameraShakeOffset(1).x).toBeCloseTo(0, 6);
    expect(cameraShakeOffset(1).y).toBeCloseTo(0, 6);
    // 来回颤，不是往一个方向推
    expect(cameraShakeOffset(0.3).y).toBeLessThan(0);
  });

  it("颤完把相机放回基准位", () => {
    const runtime = {
      camera: { position: new THREE.Vector3(0, 0, 0) },
      cameraBase: new THREE.Vector3(0, 21, 18.2),
      shake: { startedAt: 1000, duration: 200, amplitude: 0.2 },
    } as unknown as TableRuntime;
    // 颤动期间偏离基准位
    expect(advanceCameraShake(runtime, 1100)).not.toBeNull();
    expect(runtime.camera.position.y).not.toBeCloseTo(21, 6);
    // 结束后回到基准位，不留残余偏移
    expect(advanceCameraShake(runtime, 1200)).toBeNull();
    expect(runtime.camera.position.toArray()).toEqual([0, 21, 18.2]);
  });
});
