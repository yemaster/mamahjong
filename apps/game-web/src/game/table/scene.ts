import * as THREE from "three";
import type { MatchPlayerView, MatchView } from "../../types";
import type { OpeningPhase } from "../OpeningSequence";
import { addCenterConsole } from "./centerConsole";
import { addDiscards, updateLastDiscard } from "./discards";
import { addTableDice, DICE_SETTLE_MS, DORA_FLIP_DELAY_MS } from "./dice";
import { addHand } from "./hands";
import {
  completedImpactRinshanDraws,
  countCompletedKans,
  playerCompletedKan,
  playerExtractedNorth,
  playerIsHoldingDrawnTile,
  playerReceivedDraw,
  resolveRinshanDrawNumber,
} from "./handView";
import { addMelds } from "./melds";
import { tableRelativeSeat } from "./geometry";
import { disposeGroup } from "./runtime";
import { addSelfDraw } from "./selfMotion";
import { addTableSurface } from "./tableSurface";
import {
  applyTableTileHighlight,
  rebuildTableTileHighlights,
} from "./tileHighlight";
import type { TableRuntime } from "./types";
import {
  addWallTile,
  impactWallTiles,
  riichiWallTiles,
  type WallTileSpec,
} from "./wall";
import {
  impactWallLayout,
  riichiWallLayout,
  sanmaWallLayout,
  type WallLayout,
} from "./wallLayout";

/**
 * 手切空隙：别家从手牌里抽走一张打出去，手牌的 3D 立姿就该缺那一格，
 * 让围观的人肉眼分得出手切摸切。
 *
 * 空隙固定停留一秒，然后牌阵平滑归拢。真实牌数全程只取服务端的
 * concealed_tile_count，这里只记录“哪个槽位暂时是空的”。
 */
const HAND_CUT_GAP_HOLD_MS = 1000;

function updateHandCutGaps(
  runtime: TableRuntime,
  view: MatchView,
  previousView: MatchView | null,
): void {
  /* 冲击麻将不记摸切，tsumogiri 一律为 false，开了空隙反而每张牌都缺一格。
     只对立直麻将生效。 */
  if (view.variant_kind !== "riichi") {
    runtime.handCutGaps.clear();
    runtime.handCollapses.clear();
    return;
  }

  if (
    !previousView ||
    previousView.id !== view.id ||
    previousView.hand_index !== view.hand_index
  ) {
    runtime.handCutGaps.clear();
    runtime.handCollapses.clear();
    return;
  }

  /* 看有没有别家打出了新的手切牌。 */
  for (const player of view.players) {
    if (player.seat === view.observer_seat) continue;
    const prev = previousView.players.find(
      (candidate) => candidate.seat === player.seat,
    );
    if (!prev || player.discards.length <= prev.discards.length) continue;

    const lastDiscard = player.discards[player.discards.length - 1]!;
    if (lastDiscard.tsumogiri) continue;

    /* 独立摸入牌不参加手切；只从摸牌前的 3k+1 张基础手牌里随机抽一个槽。 */
    const holdingDrawn = playerIsHoldingDrawnTile(previousView, player.seat);
    const previousCount = holdingDrawn
      ? prev.concealed_tile_count - 1
      : prev.concealed_tile_count;
    if (previousCount <= 0) continue;

    const gapPosition = Math.floor(Math.random() * previousCount);
    const gap = {
      gapPosition,
      tileId: lastDiscard.tile.id,
    };
    runtime.handCollapses.delete(player.seat);
    runtime.handCutGaps.set(player.seat, gap);

    window.setTimeout(() => {
      if (runtime.disposed) return;
      const activeGap = runtime.handCutGaps.get(player.seat);
      /* 新一局或同家又发生了更新时，旧定时器不能碰新的牌阵。 */
      if (activeGap?.tileId !== gap.tileId) return;
      runtime.handCutGaps.delete(player.seat);
      runtime.handCollapses.set(player.seat, {
        gapPosition: gap.gapPosition,
        startedAt: performance.now(),
      });
      runtime.rebuild();
    }, HAND_CUT_GAP_HOLD_MS);
  }
}

/**
 * 把最新视图增量同步到牌桌。
 *
 * 场景按桌面、桌心、牌山、骰子，以及每家的手牌/牌河/副露拆成缓存层。服务端版本
 * 增长本身不会触发任何 Three.js 工作；只有某层真正参与绘制的数据变了才离屏重建
 * 那一小层，其他 mesh、材质、GPU buffer 和正在播放的动画都原样保留。
 */
export function renderTable(
  runtime: TableRuntime,
  view: MatchView,
  openingPhase: OpeningPhase,
  dice: [number, number],
  settlementRevealSeats: number[],
  settlementWinningTileSeats: number[] = [],
): void {
  commitLayerUpdates(runtime);
  runtime.latestView = view;
  const openingKey = `${view.id}:${view.hand_index}`;
  runtime.openingKey = openingKey;
  runtime.renderedOpeningPhase = openingPhase;
  if (runtime.tileGeometryWidthRatio !== runtime.tileWidthRatio) {
    /* 几何体缓存的 key 自带宽度，新旧宽度可以共存到 runtime 销毁；这样无需为了
       一个显示设置把仍在使用的 GPU buffer 提前释放。 */
    runtime.tileGeometryWidthRatio = runtime.tileWidthRatio;
  }
  const previousView = runtime.previousView;
  if (previousView && previousView.hand_index !== view.hand_index) {
    runtime.pendingRinshanDraws.clear();
    runtime.discardFlights.clear();
  }
  updateHandCutGaps(runtime, view, previousView);
  updateLastDiscard(runtime, view, previousView);
  const settlementHandKey = view.hand_settlement
    ? `${view.id}:${view.hand_index}`
    : null;
  if (runtime.settlementHandKey !== settlementHandKey) {
    runtime.settlementHandKey = settlementHandKey;
    runtime.revealedSettlementSeats.clear();
    runtime.revealedWinningTileSeats.clear();
  }

  updateLayer(runtime, "surface", "surface-v1", () => {
    addTableSurface(runtime);
  });
  updateLayer(runtime, "console", consoleKey(view), () => {
    addCenterConsole(runtime, view);
  });

  /* 开门位置只跟庄家和骰子有关，和谁在看这张桌子无关。 */
  const dealerRelative = tableRelativeSeat(
    view.progress.dealer,
    view.observer_seat,
    view.players.length,
  );
  const impact = view.variant_kind === "impact";
  const wall = impact
    ? impactWallLayout(view.progress.dealer, view.observer_seat, dice)
    : view.players.length === 3
      ? sanmaWallLayout(
          view.progress.dealer,
          view.observer_seat,
          dice,
          view.sanma_north_rule,
        )
      : riichiWallLayout(dealerRelative, dice);
  const completedKanCount = countCompletedKans(view);
  const completedNukiCount = view.players.reduce(
    (count, player) => count + (player.nuki_tiles?.length ?? 0),
    0,
  );
  const completedRinshanDraws =
    impact
      ? completedImpactRinshanDraws(view)
      : (view.completed_rinshan_draws ?? completedKanCount + completedNukiCount);

  /*
   * 冲击麻将的岭上来源在进入杠点动画时就记下来。不能只盯副露差分：加杠可能先
   * 经过抢杠响应，而且动画阶段的多次回执会产生若干副露完全相同的新版本。
   */
  if (impact && view.phase.kind === "awaiting_kan_animation") {
    runtime.pendingRinshanDraws.set(
      view.phase.seat,
      completedRinshanDraws + 1,
    );
  }
  for (const player of view.players) {
    const previousPlayer = previousView?.players.find(
      (candidate) => candidate.seat === player.seat,
    );
    const replacementWasPlaced =
      playerCompletedKan(player, previousPlayer) ||
      playerExtractedNorth(player, previousPlayer);
    if (replacementWasPlaced) {
      const replacementWasAlreadyDrawn = playerReceivedDraw(
        view,
        previousView,
        player,
        previousPlayer,
      );
      runtime.pendingRinshanDraws.set(
        player.seat,
        completedRinshanDraws + (replacementWasAlreadyDrawn ? 0 : 1),
      );
    }
  }
  /*
   * 山上还立着几张：立直的山尾十四张是王牌，一直立着；冲击麻将立着的是翻财神
   * 那一墩，它压根不在摸牌序列里，所以只按还没摸走的张数算。
   */
  const visibleWallTiles =
    openingPhase === "dice"
      ? wall.drawableCount
      : Math.min(
          wall.drawableCount,
          view.remaining_live_draws + (impact ? 0 : 14),
        );
  const consumedTileCount =
    wall.drawableCount - visibleWallTiles - completedRinshanDraws;
  /*
   * 顺序不能倒：骰子还在滚的时候牌山上不得已经亮着一张宝牌，所以只有掷骰这一段
   * 才把翻牌排到骰子停稳之后；其余时候（含刷新重建）宝牌本来就该是亮的。
   */
  const doraFlipAt =
    openingPhase === "dice"
      ? performance.now() + DICE_SETTLE_MS + DORA_FLIP_DELAY_MS
      : null;
  const wallTiles = impact
    ? impactWallTiles(
        wall,
        visibleWallTiles,
        completedRinshanDraws,
        view.joker_indicator,
        doraFlipAt,
      )
    : riichiWallTiles(
        wall,
        view.remaining_live_draws,
        view.dora_indicators ?? [],
        completedRinshanDraws,
        openingPhase === "dice",
        doraFlipAt,
      );
  syncWallLayers(
    runtime,
    wall,
    wallTiles,
    signature([
      dimensionsKey(runtime),
      openingKey,
      view.variant_kind,
      view.progress.dealer,
      view.observer_seat,
      view.players.length,
      dice,
      view.sanma_north_rule,
    ]),
    openingPhase === "dice" ? "delayed" : "settled",
  );

  if (openingPhase === "dice") {
    updateLayer(runtime, "dice", signature([openingKey, dice]), () => {
      addTableDice(runtime, dice);
    });
    for (const player of view.players) {
      clearPlayerLayers(runtime, player.seat, openingKey);
    }
    commitLayerUpdates(runtime);
    refreshTableTileHighlights(runtime);
    return;
  }
  updateLayer(runtime, "dice", `hidden:${openingKey}`, () => {});

  for (const player of view.players) {
    const previousPlayer = previousView?.players.find(
      (candidate) => candidate.seat === player.seat,
    );
    const pendingRinshanDraw = runtime.pendingRinshanDraws.get(player.seat);
    const receivedDraw = playerReceivedDraw(
      view,
      previousView,
      player,
      previousPlayer,
    );
    const previousCompletedRinshanDraws = previousView
      ? previousView.variant_kind === "impact"
        ? completedImpactRinshanDraws(previousView)
        : (previousView.completed_rinshan_draws ?? 0)
      : completedRinshanDraws;
    /* 正常路径从等待杠动画那一帧保存来源槽位。若客户端恰好漏掉了那份快照，接口
       的实际岭上计数跃迁仍能直接指出这是第几张，不能让牌只在山尾消失。 */
    const rinshanDrawNumber = resolveRinshanDrawNumber(
      receivedDraw,
      pendingRinshanDraw,
      completedRinshanDraws,
      previousCompletedRinshanDraws,
    );
    updateLayer(
      runtime,
      `hand:${player.seat}`,
      handKey(
        runtime,
        view,
        player,
        openingPhase,
        settlementRevealSeats,
        settlementWinningTileSeats,
      ),
      () => {
        addHand(
          runtime,
          view,
          player,
          previousPlayer,
          openingPhase,
          settlementRevealSeats,
          settlementWinningTileSeats,
          wall,
          consumedTileCount,
          rinshanDrawNumber,
        );
      },
    );
    if (settlementRevealSeats.includes(player.seat)) {
      runtime.revealedSettlementSeats.add(player.seat);
    }
    if (settlementWinningTileSeats.includes(player.seat)) {
      runtime.revealedWinningTileSeats.add(player.seat);
    }
    if (openingPhase === "play" || openingPhase === "waiting") {
      updateLayer(
        runtime,
        `discards:${player.seat}`,
        discardKey(runtime, view, player, openingPhase),
        () => addDiscards(runtime, view, player, previousPlayer, openingPhase),
      );
    } else {
      updateLayer(runtime, `discards:${player.seat}`, `hidden:${openingKey}`, () => {});
    }
    if (openingPhase === "play") {
      updateLayer(
        runtime,
        `melds:${player.seat}`,
        meldKey(runtime, view, player),
        () => addMelds(runtime, view, player, previousPlayer),
      );
    } else {
      updateLayer(runtime, `melds:${player.seat}`, `hidden:${openingKey}`, () => {});
    }
    if (player.seat === view.observer_seat) {
      updateLayer(
        runtime,
        "self-draw",
        signature([
          dimensionsKey(runtime),
          openingKey,
          openingPhase,
          runtime.instantDraw,
          player.drawn_tile_id,
          player.concealed_tiles,
          rinshanDrawNumber,
          view.phase.kind,
          view.joker_code,
          view.dora_indicators,
        ]),
        () =>
          addSelfDraw(
            runtime,
            view,
            player,
            previousPlayer,
            openingPhase,
            wall,
            consumedTileCount,
            rinshanDrawNumber,
          ),
      );
    }
    if (rinshanDrawNumber != null) {
      runtime.pendingRinshanDraws.delete(player.seat);
    }
  }
  runtime.previousView = view;
  commitLayerUpdates(runtime);
  refreshTableTileHighlights(runtime);
}

function signature(value: unknown): string {
  return JSON.stringify(value);
}

function dimensionsKey(runtime: TableRuntime): string {
  return `${runtime.tileScale}:${runtime.tileWidthRatio}`;
}

function consoleKey(view: MatchView): string {
  return signature([
    view.id,
    view.hand_index,
    view.variant_kind,
    view.progress,
    view.remaining_live_draws,
    view.observer_seat,
    view.dealer_streak,
    view.players.map((player) => [
      player.seat,
      player.points,
      player.riichi_status,
    ]),
  ]);
}

function handKey(
  runtime: TableRuntime,
  view: MatchView,
  player: MatchPlayerView,
  openingPhase: OpeningPhase,
  settlementRevealSeats: number[],
  settlementWinningTileSeats: number[],
): string {
  return signature([
    dimensionsKey(runtime),
    view.id,
    view.hand_index,
    openingPhase,
    player.seat,
    player.concealed_tile_count,
    player.concealed_tiles,
    player.drawn_tile_id,
    playerIsHoldingDrawnTile(view, player.seat),
    view.phase.kind === "ended",
    view.hand_settlement,
    settlementRevealSeats.includes(player.seat),
    settlementWinningTileSeats.includes(player.seat),
    runtime.revealAllHands,
    runtime.handCutGaps.get(player.seat),
    runtime.handCollapses.get(player.seat),
    view.dora_indicators,
    view.joker_code,
  ]);
}

function discardKey(
  runtime: TableRuntime,
  view: MatchView,
  player: MatchPlayerView,
  openingPhase: OpeningPhase,
): string {
  const marker =
    runtime.lastDiscard?.seat === player.seat
      ? runtime.lastDiscard.index
      : null;
  return signature([
    dimensionsKey(runtime),
    view.id,
    view.hand_index,
    openingPhase,
    player.discards,
    player.nuki_tiles,
    marker,
    runtime.dimTsumogiri,
    view.dora_indicators,
    view.joker_code,
  ]);
}

function meldKey(
  runtime: TableRuntime,
  view: MatchView,
  player: MatchPlayerView,
): string {
  return signature([
    dimensionsKey(runtime),
    view.id,
    view.hand_index,
    player.melds,
    view.dora_indicators,
    view.joker_code,
  ]);
}

function clearPlayerLayers(
  runtime: TableRuntime,
  seat: number,
  openingKey: string,
): void {
  for (const kind of ["hand", "discards", "melds"]) {
    updateLayer(runtime, `${kind}:${seat}`, `hidden:${openingKey}`, () => {});
  }
  updateLayer(runtime, "self-draw", `hidden:${openingKey}`, () => {});
}

/** 牌山按物理槽位同步：正常摸牌只会让一个槽位变空，不碰其余一百多张牌。 */
function syncWallLayers(
  runtime: TableRuntime,
  layout: WallLayout,
  tiles: WallTileSpec[],
  layoutKey: string,
  flipMode: "delayed" | "settled",
): void {
  const visible = new Set<string>();
  for (const tile of tiles) {
    const key = `wall-slot:${tile.slot}`;
    visible.add(key);
    updateLayer(
      runtime,
      key,
      signature([layoutKey, tile.slot, tile.code, tile.flipAt ? flipMode : null]),
      () => addWallTile(runtime, layout, tile.slot, tile.code, tile.flipAt),
    );
  }
  for (const key of runtime.layers.keys()) {
    if (!key.startsWith("wall-slot:") || visible.has(key)) continue;
    updateLayer(runtime, key, `empty:${layoutKey}`, () => {});
  }
}

/** 只替换一个发生变化的视觉层；同一 JS 帧内完成挂载和撤换，不暴露空场景。 */
function updateLayer(
  runtime: TableRuntime,
  key: string,
  nextSignature: string,
  build: () => void,
): void {
  const previous = runtime.layers.get(key);
  if (previous?.signature === nextSignature) return;

  const group = new THREE.Group();
  group.name = `table-layer:${key}`;
  runtime.renderTarget = group;
  try {
    build();
  } finally {
    runtime.renderTarget = runtime.root;
  }

  runtime.root.add(group);
  if (key.startsWith("discards:") || key.startsWith("melds:")) {
    runtime.highlightIndexDirty = true;
  }
  if (previous) {
    runtime.pendingLayerDisposals.push(previous.group);
  }
  runtime.layers.set(key, { signature: nextSignature, group });
}

function refreshTableTileHighlights(runtime: TableRuntime): void {
  if (!runtime.highlightIndexDirty) return;
  rebuildTableTileHighlights(runtime);
  applyTableTileHighlight(runtime);
  runtime.highlightIndexDirty = false;
}

/** 一份视图涉及的所有局部层一次提交；下一次 rAF 只会看见提交后的完整场景。 */
function commitLayerUpdates(runtime: TableRuntime): void {
  if (runtime.pendingLayerDisposals.length === 0) return;
  for (const group of runtime.pendingLayerDisposals) {
    removeLayerRuntimeState(runtime, group);
    runtime.root.remove(group);
    disposeGroup(group);
    group.clear();
  }
  runtime.pendingLayerDisposals = [];
}

/** 去掉被替换子树留下的动画、拾取与临时特效引用。 */
function removeLayerRuntimeState(
  runtime: TableRuntime,
  group: THREE.Group,
): void {
  const belongs = (object: THREE.Object3D): boolean => {
    let current: THREE.Object3D | null = object;
    while (current) {
      if (current === group) return true;
      current = current.parent;
    }
    return false;
  };
  runtime.selectable = runtime.selectable.filter((mesh) => !belongs(mesh));
  if (runtime.hovered && belongs(runtime.hovered)) runtime.hovered = null;
  runtime.animations = runtime.animations.filter(
    (animation) => !belongs(animation.group),
  );
  runtime.tilts = runtime.tilts.filter((tilt) => !belongs(tilt.group));
  runtime.spinners = runtime.spinners.filter((spinner) => !belongs(spinner));
  runtime.transients = runtime.transients.filter(
    (transient) => !belongs(transient.group),
  );
  runtime.impacts = runtime.impacts.filter((impact) => !belongs(impact.mesh));
  runtime.diceRolls = runtime.diceRolls.filter(
    (roll) => !belongs(roll.object),
  );
}
