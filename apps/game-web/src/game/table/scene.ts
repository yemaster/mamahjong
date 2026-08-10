import type { MatchView } from "../../types";
import type { OpeningPhase } from "../OpeningSequence";
import { addCenterConsole } from "./centerConsole";
import { addDiscards, updateLastDiscard } from "./discards";
import { addTableDice, DICE_SETTLE_MS, DORA_FLIP_DELAY_MS } from "./dice";
import { addHand } from "./hands";
import { countCompletedKans, playerIsHoldingDrawnTile } from "./handView";
import { addMelds } from "./melds";
import { tableRelativeSeat } from "./geometry";
import { disposeGroup } from "./runtime";
import { addSelfDraw } from "./selfMotion";
import { addTableSurface } from "./tableSurface";
import { applyTableTileHighlight } from "./tileHighlight";
import type { TableRuntime } from "./types";
import { addImpactWall, addWall } from "./wall";
import { impactWallLayout, riichiWallLayout } from "./wallLayout";

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
 * 按最新的对局视图重建整张桌子。
 *
 * 每次都是推倒重来：清空场景再逐块搭回去，需要动画的部分由各个 add* 自己
 * 对着上一份视图做差分（新打出的牌、新成立的副露……）后登记到 runtime 上。
 */
export function renderTable(
  runtime: TableRuntime,
  view: MatchView,
  openingPhase: OpeningPhase,
  dice: [number, number],
  settlementRevealSeats: number[],
  settlementWinningTileSeats: number[] = [],
): void {
  runtime.latestView = view;
  const openingKey = `${view.id}:${view.hand_index}`;
  if (
    openingPhase !== "play" &&
    runtime.renderedOpeningPhase === openingPhase &&
    runtime.openingKey === openingKey
  ) {
    runtime.previousView = view;
    return;
  }
  runtime.openingKey = openingKey;
  runtime.renderedOpeningPhase = openingPhase;
  disposeGroup(runtime.root);
  runtime.root.clear();
  runtime.selectable = [];
  runtime.hovered = null;
  runtime.animations = [];
  runtime.tilts = [];
  runtime.spinners = [];
  /* 桌子推倒重来，还没散完的灰也跟着没了；镜头也得先稳住。 */
  runtime.impacts = [];
  runtime.transients = [];
  runtime.shake = null;
  runtime.highlightMaterials = new Map();
  runtime.diceRolls = [];
  runtime.centerConsoleMesh = null;
  const previousView = runtime.previousView;
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

  addTableSurface(runtime);
  addCenterConsole(runtime, view);

  /* 开门位置只跟庄家和骰子有关，和谁在看这张桌子无关。 */
  const dealerRelative = tableRelativeSeat(
    view.progress.dealer,
    view.observer_seat,
    view.players.length,
  );
  const impact = view.variant_kind === "impact";
  const wall = impact
    ? impactWallLayout(view.progress.dealer, view.observer_seat, dice)
    : riichiWallLayout(dealerRelative, dice);
  const completedKanCount = countCompletedKans(view);
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
    wall.drawableCount - visibleWallTiles - completedKanCount;
  /*
   * 顺序不能倒：骰子还在滚的时候牌山上不得已经亮着一张宝牌，所以只有掷骰这一段
   * 才把翻牌排到骰子停稳之后；其余时候（含刷新重建）宝牌本来就该是亮的。
   */
  const doraFlipAt =
    openingPhase === "dice"
      ? performance.now() + DICE_SETTLE_MS + DORA_FLIP_DELAY_MS
      : null;
  if (impact) {
    addImpactWall(
      runtime,
      wall,
      visibleWallTiles,
      completedKanCount,
      view.joker_indicator,
      doraFlipAt,
    );
  } else {
    addWall(
      runtime,
      wall,
      visibleWallTiles,
      view.dora_indicators ?? [],
      view.players.length,
      completedKanCount,
      doraFlipAt,
    );
  }

  if (openingPhase === "dice") {
    addTableDice(runtime, dice);
    return;
  }

  for (const player of view.players) {
    const previousPlayer = previousView?.players.find(
      (candidate) => candidate.seat === player.seat,
    );
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
    );
    if (settlementRevealSeats.includes(player.seat)) {
      runtime.revealedSettlementSeats.add(player.seat);
    }
    if (settlementWinningTileSeats.includes(player.seat)) {
      runtime.revealedWinningTileSeats.add(player.seat);
    }
    if (openingPhase === "play" || openingPhase === "waiting") {
      addDiscards(runtime, view, player, previousPlayer, openingPhase);
    }
    if (openingPhase === "play") {
      addMelds(runtime, view, player, previousPlayer);
    }
    if (player.seat === view.observer_seat) {
      addSelfDraw(
        runtime,
        view,
        player,
        previousPlayer,
        openingPhase,
        wall,
        consumedTileCount,
      );
    }
  }
  /* 桌子是推倒重来的，手上还拿着牌就得把点亮重新刷上去。 */
  applyTableTileHighlight(runtime);
  runtime.previousView = view;
}
