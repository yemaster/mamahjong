import * as THREE from "three";
import type { MatchView } from "../../types";
import type { OpeningPhase } from "../OpeningSequence";
import { coveredHandTilt, standingHandTilt } from "./animation";
import { TILE_DEPTH_RATIO, TILE_LENGTH } from "./constants";
import {
  exchangeStackPosition,
  handPosition,
  tableRelativeSeat,
} from "./geometry";
import {
  playerIsHoldingDrawnTile,
  sortHandForDisplay,
} from "./handView";
import { opponentHandLayout } from "./opponentHandMotion";
/* scene 也导入本模块，这里成环；两边都只在函数体里调用，ESM 活绑定下安全。 */
import { updateLayer } from "./scene";
import { makeTile } from "./tileMesh";
import type { TableRuntime } from "./types";

/**
 * 四川麻将「换三张」的三维演出。
 *
 * 选牌在二维手牌里完成；提交之后这里才接手：照着提交那一刻抓下的快照把手牌立在
 * 桌上，交出去的三张飞到牌河中央盖住，别家各自飞出三张，四摞牌按骰子方向换位；
 * 别家把换入牌飞回手牌，主视角则在收束时直接交还二维手牌。
 *
 * 阶段推进**挂在 runtime 的 rAF 动画循环上**（`advanceExchange`），不用一次性
 * 定时器——那样一旦哪个回调被清掉整段演出就僵死。每一段之间留足停顿，看得清。
 */

export type ExchangeDirection = "counter_clockwise" | "clockwise" | "opposite";

/** React 提交换牌时抓下的快照：换前整手与交出去的三张。 */
export interface ExchangeSnapshot {
  /** 防止 React 状态清理晚于新局第一帧时，把上一局快照重新交给牌桌。 */
  handKey: string;
  hand: { id: number; code: string }[];
  outgoingIds: number[];
}

type ExchangeStage = "selfFlyout" | "othersFlyout" | "swap" | "flyIn";

interface ExchangeStack {
  tiles: THREE.Group[];
  /** 真 = 正面牌坯（主视角交出去的那一摞）；假 = 牌背。 */
  face: boolean;
}

export interface ExchangeState {
  key: string;
  stage: ExchangeStage;
  /** 本阶段开始的时刻；`advanceExchange` 照它算流逝时间。 */
  stageStartedAt: number;
  direction: ExchangeDirection;
  snapshot: ExchangeSnapshot;
  /** 主视角手牌节点，按牌 id 索引（含已飞出的三张）。 */
  selfTiles: Map<number, THREE.Group>;
  /** 各家飞出的三张；换位之后键换成接收家。 */
  stacks: Map<number, ExchangeStack>;
}

/* ── 时长 ──
 * 飞出阶段的三张牌同帧起飞；只有跨座换位和飞回手牌保留轻微的逐张节奏。
 * 整段约 4.5 秒，动作保持清楚但节奏更利落。 */
/** 一张牌从手牌飞到牌河中央的时长。 */
const FLY_OUT_MS = 600;
/** 四摞牌跨座换位的时长。 */
const SWAP_MS = 900;
/** 换好的牌从牌河飞入手牌的时长。 */
const FLY_IN_MS = 600;
/** 手牌抽走三张后并拢的时长。 */
const HAND_COLLAPSE_MS = 420;
/** 三张牌是同一组换牌，飞出、换位和飞回都必须同帧开始。 */
const STACK_MOVE_STAGGER_MS = 0;
/** 段末停顿：让每一步都看得清再继续。 */
const SELF_FLYOUT_HOLD_MS = 350;
const OTHERS_FLYOUT_HOLD_MS = 400;
const SWAP_HOLD_MS = 450;
const FLY_IN_HOLD_MS = 300;

const SELF_FLYOUT_STAGE_MS = FLY_OUT_MS + SELF_FLYOUT_HOLD_MS;
const OTHERS_FLYOUT_STAGE_MS = FLY_OUT_MS + OTHERS_FLYOUT_HOLD_MS;
const SWAP_STAGE_MS = SWAP_MS + STACK_MOVE_STAGGER_MS * 2 + SWAP_HOLD_MS;
const FLY_IN_STAGE_MS = FLY_IN_MS + STACK_MOVE_STAGGER_MS * 2 + FLY_IN_HOLD_MS;

/** 整段演出总时长；React 用它做兜底回执的等待上限。 */
export const EXCHANGE_CINEMATIC_MS =
  SELF_FLYOUT_STAGE_MS +
  OTHERS_FLYOUT_STAGE_MS +
  SWAP_STAGE_MS +
  FLY_IN_STAGE_MS;

const X_AXIS = new THREE.Vector3(1, 0, 0);
const Y_AXIS = new THREE.Vector3(0, 1, 0);

/**
 * 这个方向下每家把牌传给谁。和服务端 `ExchangeDirection::recipient_of` 一致：
 * 逆时针 0→1→2→3→0、顺时针 0→3→2→1→0、对家 0↔2 / 1↔3。
 */
export function exchangeRecipient(
  seat: number,
  direction: ExchangeDirection,
): number {
  switch (direction) {
    case "counter_clockwise":
      return (seat + 1) % 4;
    case "clockwise":
      return (seat + 3) % 4;
    case "opposite":
      return seat ^ 2;
  }
}

/**
 * 换牌阶段每一帧视图同步的入口。
 *
 * 把 React 传来的提交快照转存到 runtime（`advanceExchange` 在 rAF 循环里随时读），
 * 并按阶段是否活跃决定要不要清理演出。返回真表示主视角手牌层此刻归本模块管，
 * `renderTable` 不再对它做常规同步。真正的起手与推进都在 `advanceExchange`。
 */
export function updateExchange(
  runtime: TableRuntime,
  view: MatchView,
  openingPhase: OpeningPhase,
  snapshot: ExchangeSnapshot | null,
): boolean {
  if (view.variant_kind !== "sichuan") {
    if (runtime.exchange) cleanupExchange(runtime);
    runtime.exchangeCompletedKey = null;
    runtime.exchangeSnapshot = null;
    runtime.exchangeSnapshotKey = null;
    return false;
  }
  const key = `${view.id}:${view.hand_index}`;
  if (runtime.exchange && runtime.exchange.key !== key) {
    cleanupExchange(runtime);
  }
  if (runtime.exchangeCompletedKey !== key) {
    runtime.exchangeCompletedKey = null;
  }
  /* 服务端已经记下本家的动画回执时，这一局绝不能因为旧快照重连再演一次。 */
  if ((view.exchange_animation_played_seats ?? []).includes(view.observer_seat)) {
    runtime.exchangeCompletedKey = key;
  }
  if (runtime.exchangeSnapshotKey !== key) {
    runtime.exchangeSnapshot = null;
    runtime.exchangeSnapshotKey = key;
  }
  if (snapshot?.handKey === key) runtime.exchangeSnapshot = snapshot;

  if (
    !exchangePhaseActive(view, openingPhase) &&
    !(runtime.exchange && view.phase.kind === "awaiting_dingque")
  ) {
    if (runtime.exchange) cleanupExchange(runtime);
    return false;
  }

  maybeActivate(runtime, view, key);
  return runtime.exchange != null;
}

/** 换牌演出是否该在跑：川麻、开局动画放行、处在换牌/定缺且本家还没回执。 */
function exchangePhaseActive(view: MatchView, openingPhase: OpeningPhase): boolean {
  if (view.variant_kind !== "sichuan" || openingPhase !== "play") return false;
  const phase = view.phase.kind;
  const played = (view.exchange_animation_played_seats ?? []).includes(
    view.observer_seat,
  );
  return (
    phase === "awaiting_exchange" ||
    phase === "awaiting_exchange_animation" ||
    (phase === "awaiting_dingque" && !played)
  );
}

/** 有快照、阶段活跃且演出未起：起手。幂等，rAF 与视图同步都可调。 */
function maybeActivate(runtime: TableRuntime, view: MatchView, key: string): void {
  if (runtime.exchange || runtime.exchangeCompletedKey === key) return;
  const snapshot = runtime.exchangeSnapshot;
  if (!snapshot) return;
  activateCinematic(runtime, view, key, snapshot);
}

/**
 * rAF 循环每帧调用：先补起手（视图同步那一帧没赶上时在这儿起），再按流逝时间把
 * 阶段往前推。条件没凑齐（例如自己的三张落桌后还在等四家交完）就原地等着，
 * 下一帧再试——不靠一次性定时器，不会僵死。
 */
export function advanceExchange(runtime: TableRuntime, now: number): void {
  if (runtime.disposed) return;
  const view = runtime.latestView;
  if (!view) return;

  const state = runtime.exchange;
  if (!state) {
    const openingPhase = runtime.renderedOpeningPhase;
    if (openingPhase != null && exchangePhaseActive(view, openingPhase)) {
      maybeActivate(runtime, view, `${view.id}:${view.hand_index}`);
    }
    return;
  }

  const elapsed = now - state.stageStartedAt;
  switch (state.stage) {
    case "selfFlyout":
      /* 自己的三张落桌后，还要等四家都交完（进入定缺阶段）才演别家。 */
      if (
        elapsed >= SELF_FLYOUT_STAGE_MS &&
        (view.phase.kind === "awaiting_exchange_animation" ||
          view.phase.kind === "awaiting_dingque")
      ) {
        startOthersFlyout(runtime, view, state, now);
      }
      break;
    case "othersFlyout":
      if (elapsed >= OTHERS_FLYOUT_STAGE_MS) startSwap(runtime, view, state, now);
      break;
    case "swap":
      if (elapsed >= SWAP_STAGE_MS) startFlyIn(runtime, view, state, now);
      break;
    case "flyIn":
      if (elapsed >= FLY_IN_STAGE_MS) finishExchange(runtime, state);
      break;
  }
}

/** 提交之后起手：照快照把整手立在桌上，立刻飞出交出去的三张。 */
function activateCinematic(
  runtime: TableRuntime,
  view: MatchView,
  key: string,
  snapshot: ExchangeSnapshot,
): void {
  const state: ExchangeState = {
    key,
    stage: "selfFlyout",
    stageStartedAt: performance.now(),
    direction: view.exchange_direction ?? "counter_clockwise",
    snapshot,
    selfTiles: new Map(),
    stacks: new Map(),
  };
  runtime.exchange = state;
  buildHand(runtime, view.observer_seat, state);
  startSelfFlyout(runtime, view, state, state.stageStartedAt);
}

/** 照快照把主视角整手牌立在桌上：正面、和别家手牌同一条基线。 */
function buildHand(
  runtime: TableRuntime,
  observerSeat: number,
  state: ExchangeState,
): void {
  const tiles = state.snapshot.hand;
  const outgoing = new Set(state.snapshot.outgoingIds);
  updateLayer(
    runtime,
    `hand:${observerSeat}`,
    `exchange:${state.key}`,
    () => {
      tiles.forEach((tile, index) => {
        /* 主视角整手牌继续由二维层展示；三维层只创建真正离手的三张。 */
        if (!outgoing.has(tile.id)) return;
        const group = makeTile(runtime, tile.code, TILE_LENGTH);
        const position = handPosition(
          0,
          tiles.length,
          index,
          true,
          0,
          false,
          runtime.tileWidthRatio,
          runtime.tileScale,
        );
        group.position.copy(position);
        (group.userData.tilePivot as THREE.Group).rotation.x =
          standingHandTilt(false);
        group.userData.baseY = position.y;
        group.userData.tileId = tile.id;
        state.selfTiles.set(tile.id, group);
        runtime.renderTarget.add(group);
      });
    },
  );
}

/** 第一段：交出去的三张飞到自己牌河中央盖住，其余手牌原地并拢。 */
function startSelfFlyout(
  runtime: TableRuntime,
  view: MatchView,
  state: ExchangeState,
  now: number,
): void {
  state.stage = "selfFlyout";
  state.stageStartedAt = now;
  const outgoing = new Set(state.snapshot.outgoingIds);
  const preHand = state.snapshot.hand;

  const remaining = preHand.filter((tile) => !outgoing.has(tile.id));
  remaining.forEach((tile, index) => {
    const group = state.selfTiles.get(tile.id);
    if (!group) return;
    const destination = handPosition(
      0,
      remaining.length,
      index,
      true,
      0,
      false,
      runtime.tileWidthRatio,
      runtime.tileScale,
    );
    runtime.animations.push({
      group,
      start: group.position.clone(),
      end: destination,
      startedAt: now,
      duration: HAND_COLLAPSE_MS,
    });
  });

  const flying = preHand.filter((tile) => outgoing.has(tile.id));
  const groups: THREE.Group[] = [];
  flying.forEach((tile, index) => {
    const group = state.selfTiles.get(tile.id);
    if (!group) return;
    const start = group.position.clone();
    const end = exchangeStackSlot(runtime, 0, index, true);
    const startRotation = bakePivotRotation(group);
    const endRotation = flatQuaternion(0, false);
    runtime.animations.push({
      group,
      start,
      end,
      startRotation,
      endRotation,
      /* 三张一起拿出，不做逐张错开。 */
      startedAt: now,
      duration: FLY_OUT_MS,
      arcHeight: 0.7,
    });
    groups.push(group);
  });
  state.stacks.set(view.observer_seat, { tiles: groups, face: true });
}

/** 第二段：其余三家各从手里抽三张牌背飞到自家牌河中央。 */
function startOthersFlyout(
  runtime: TableRuntime,
  view: MatchView,
  state: ExchangeState,
  now: number,
): void {
  state.stage = "othersFlyout";
  state.stageStartedAt = now;
  for (const player of view.players) {
    if (player.seat === view.observer_seat) continue;
    const relative = tableRelativeSeat(
      player.seat,
      view.observer_seat,
      view.players.length,
    );
    const pool = opponentHandPool(runtime, player.seat);
    if (pool.length < 3) continue;
    /* 服务端不透露别家选了哪三张，随机抽三张演给主视角看。 */
    const shuffled = [...pool].sort(() => Math.random() - 0.5);
    const chosen = shuffled.slice(0, 3);
    const chosenSet = new Set(chosen);
    const remaining = pool
      .filter((group) => !chosenSet.has(group))
      .sort(byOpponentHandIndex);
    remaining.forEach((group, index) => {
      const destination = handPosition(
        relative,
        remaining.length,
        index,
        false,
        0,
        false,
        runtime.tileWidthRatio,
        runtime.tileScale,
      );
      runtime.animations.push({
        group,
        start: group.position.clone(),
        end: destination,
        startedAt: now,
        duration: HAND_COLLAPSE_MS,
      });
    });
    chosen.forEach((group, index) => {
      /* 离手的牌退出普通对象池；演出结束后按权威视图整排重建。 */
      group.userData.opponentHandPool = false;
      const start = group.position.clone();
      const end = exchangeStackSlot(runtime, relative, index);
      const startRotation = bakePivotRotation(group);
      const endRotation = flatQuaternion(relative, true);
      runtime.animations.push({
        group,
        start,
        end,
        startRotation,
        endRotation,
        /* 四家拿出的三张牌同时起飞。 */
        startedAt: now,
        duration: FLY_OUT_MS,
        arcHeight: 0.7,
      });
    });
    state.stacks.set(player.seat, { tiles: chosen, face: false });
  }
}

/** 第三段：四摞牌沿换牌方向飞到接收家的牌河中央。 */
function startSwap(
  runtime: TableRuntime,
  view: MatchView,
  state: ExchangeState,
  now: number,
): void {
  state.stage = "swap";
  state.stageStartedAt = now;
  const moved = new Map<number, ExchangeStack>();
  for (const [seat, stack] of state.stacks) {
    const recipient = exchangeRecipient(seat, state.direction);
    const toRelative = tableRelativeSeat(
      recipient,
      view.observer_seat,
      view.players.length,
    );
    stack.tiles.forEach((group, index) => {
      const start = group.position.clone();
      const end = exchangeStackSlot(runtime, toRelative, index, stack.face);
      const startRotation = group.quaternion.clone();
      const endRotation = flatQuaternion(toRelative, !stack.face);
      runtime.animations.push({
        group,
        start,
        end,
        startRotation,
        endRotation,
        startedAt: now + index * STACK_MOVE_STAGGER_MS,
        duration: SWAP_MS,
        arcHeight: 1.05,
      });
    });
    moved.set(recipient, stack);
  }
  state.stacks = moved;
}

/** 第四段：换好的牌飞入各家手牌。 */
function startFlyIn(
  runtime: TableRuntime,
  view: MatchView,
  state: ExchangeState,
  now: number,
): void {
  state.stage = "flyIn";
  state.stageStartedAt = now;
  for (const [seat, stack] of state.stacks) {
    if (seat === view.observer_seat) {
      flyInSelf(runtime, view, state, stack, now);
    } else {
      flyInOpponent(runtime, view, seat, stack, now);
    }
  }
}

function flyInSelf(
  runtime: TableRuntime,
  view: MatchView,
  state: ExchangeState,
  stack: ExchangeStack,
  now: number,
): void {
  const observer = view.players.find(
    (player) => player.seat === view.observer_seat,
  );
  const postHand = sortHandForDisplay(observer?.concealed_tiles ?? [], null);
  const slotById = new Map(postHand.map((tile, index) => [tile.id, index]));
  const preIds = new Set(state.snapshot.hand.map((tile) => tile.id));
  const incoming = postHand.filter((tile) => !preIds.has(tile.id));
  /* 手里剩下的牌先挪到换后整手的位置，给飞进来的三张让出空当。 */
  for (const [tileId, group] of state.selfTiles) {
    if (!preIds.has(tileId)) continue;
    const slot = slotById.get(tileId);
    if (slot == null) continue;
    const destination = handPosition(
      0,
      postHand.length,
      slot,
      true,
      0,
      false,
      runtime.tileWidthRatio,
      runtime.tileScale,
    );
    runtime.animations.push({
      group,
      start: group.position.clone(),
      end: destination,
      startedAt: now,
      duration: FLY_IN_MS,
    });
  }
  /*
   * 主视角不再把换入牌用三维模型飞进手牌。二维手牌会在本段结束后一次性接手，
   * 因而这里把临时牌节点从原来的牌层摘下并隐藏，避免同一张牌在桌面上出现两份。
   */
  for (const [index, tile] of incoming.entries()) {
    const group = stack.tiles[index];
    const slot = slotById.get(tile.id);
    if (!group || slot == null) continue;
    runtime.animations = runtime.animations.filter(
      (animation) => animation.group !== group,
    );
    group.visible = false;
    group.parent?.remove(group);
  }
}

function flyInOpponent(
  runtime: TableRuntime,
  view: MatchView,
  seat: number,
  stack: ExchangeStack,
  now: number,
): void {
  const player = view.players.find((candidate) => candidate.seat === seat);
  if (!player) return;
  const relative = tableRelativeSeat(
    seat,
    view.observer_seat,
    view.players.length,
  );
  /* 先数清楚留在这家的牌；换入三张始终是临时演出节点。 */
  const pool = opponentHandPool(runtime, seat).sort(byOpponentHandIndex);
  /*
   * 手牌的横向基线必须和立直 `addHand` 一样由服务端牌数决定。动画对象池可能
   * 因为上一帧的重建暂时少/多一张；用 pool.length 会把相对 1/3 家的横向误差
   * 旋转成 z 轴偏移，正是换入三张落位错开的来源。
   */
  const count = Math.max(3, player.concealed_tile_count);
  /*
   * 和普通 `addHand` 使用同一份槽位布局。手切空位或独立摸牌槽会让可见槽位编号
   * 不再是简单的 0..count-1；相对 1/3 家旋转后，这个编号差异会直接表现成 z 轴
   * 偏移。换牌飞入不能重新发明一套槽位算法。
   */
  const layout = opponentHandLayout(
    count,
    playerIsHoldingDrawnTile(view, seat),
    runtime.handCutGaps.get(seat)?.gapPosition,
  );
  /*
   * 交换牌不能塞进接收家的对象池。对象池被立直/冲击的摸牌逻辑当作标准牌背复用，
   * 主视角交出去的正面牌坯一旦混进去，对家下一次摸牌就会出现正反翻转。临时节点
   * 留在原来的牌层里，只改变动画落点；收束后由 addHand 按权威视图整排重建。
   */
  const localPosition = (
    group: THREE.Group,
    world: THREE.Vector3,
  ): THREE.Vector3 =>
    group.parent ? group.parent.worldToLocal(world.clone()) : world.clone();
  /* 换进来的牌按规则直接接在手牌最右侧，不再把原手牌拆开插空。 */
  const incomingSlots = layout.renderedSlots.slice(-stack.tiles.length);
  const incomingSlotSet = new Set(incomingSlots);
  const restSlots = layout.renderedSlots.filter(
    (slot) => !incomingSlotSet.has(slot),
  );
  pool.forEach((group, index) => {
    const slot = restSlots[index];
    if (slot == null) return;
    group.userData.opponentHandTileIndex = slot;
    const destination = localPosition(
      group,
      handPosition(
        relative,
        layout.slotCount,
        slot,
        false,
        0,
        false,
        runtime.tileWidthRatio,
        runtime.tileScale,
      ),
    );
    const worldStart = group.getWorldPosition(new THREE.Vector3());
    runtime.animations.push({
      group,
      start: localPosition(group, worldStart),
      end: destination,
      startedAt: now,
      duration: FLY_IN_MS,
    });
  });
  stack.tiles.forEach((group, index) => {
    const slot = incomingSlots[index];
    if (slot == null) return;
    group.userData.opponentHandTileIndex = slot;
    const start = localPosition(
      group,
      group.getWorldPosition(new THREE.Vector3()),
    );
    const end = localPosition(
      group,
      handPosition(
        relative,
        layout.slotCount,
        slot,
        false,
        0,
        false,
        runtime.tileWidthRatio,
        runtime.tileScale,
      ),
    );
    const startRotation = group.quaternion.clone();
    /*
     * 主视角交出去的那一摞是正面牌坯：立在别家手牌位、牌面朝向那家主人，
     * 主视角看到的就是绿色牌背（绿色那层和牌背牌同厚），不必换成牌背牌坯。
     * 牌背牌坯照常规牌背立姿落位。
     */
    const endRotation = standingQuaternion(relative, !stack.face);
    runtime.animations.push({
      group,
      start,
      end,
      startRotation,
      endRotation,
      startedAt: now + index * STACK_MOVE_STAGGER_MS,
      duration: FLY_IN_MS,
      arcHeight: 0.6,
    });
  });
}

/**
 * 收束：别家飞入的牌恢复成常规手牌的姿态（group 只管座向、pivot 管立姿），
 * 主视角的临时三维牌层撤掉，继续由二维手牌接手。
 */
function finishExchange(runtime: TableRuntime, state: ExchangeState): void {
  const view = runtime.latestView;
  if (view) {
    for (const [seat, stack] of state.stacks) {
      if (seat === view.observer_seat) continue;
      const player = view.players.find((candidate) => candidate.seat === seat);
      if (!player) continue;
      const relative = tableRelativeSeat(
        seat,
        view.observer_seat,
        view.players.length,
      );
      for (const group of stack.tiles) {
        /* 飞行动画把立姿并进了 group 的四元数，归位时拆回 pivot，和常规手牌一致。 */
        group.quaternion.setFromAxisAngle(Y_AXIS, relative * (Math.PI / 2));
        (group.userData.tilePivot as THREE.Group).rotation.x =
          standingHandTilt(!stack.face);
        const tileIndex = group.userData.opponentHandTileIndex as
          | number
          | undefined;
        if (tileIndex != null && tileIndex >= 0) {
          /* 收束时再按常规手牌槽位校正一次，避免右侧座位因动画帧落差偏离基线。 */
          const destination = handPosition(
              relative,
              Math.max(3, player.concealed_tile_count),
              tileIndex,
              false,
              0,
              false,
              runtime.tileWidthRatio,
              runtime.tileScale,
            );
          const parent = group.parent;
          group.position.copy(
            parent ? parent.worldToLocal(destination.clone()) : destination,
          );
        }
        group.userData.baseY = group.position.y;
        /* 这仍是交换演出的临时牌，不得进入普通暗手对象池。 */
        group.userData.opponentHandPool = false;
      }
      /*
       * 动画期间复用了旧的牌背节点，但服务端此时已经把换入牌算进了整手。把
       * 该层标成待重建，下一次视图同步会用权威的手牌数量/槽位重新排一遍，
       * 不让动画过程中的临时索引或父节点坐标残留到下一巡。
       */
      const layer = runtime.layers.get(`hand:${seat}`);
      if (layer) layer.signature = "";
      /* 交换来的三张使用了原持有者的牌坯（其中主视角交出的牌是正面牌坯），
         不能把这些临时节点继续当作普通暗手对象池。下一次视图同步必须按权威
         四川手牌重新创建牌背，否则该家下一次摸牌会把正面牌坯翻成反面。 */
      runtime.forceHandRebuildSeats.add(seat);
    }
    updateLayer(
      runtime,
      `hand:${view.observer_seat}`,
      `exchange-done:${state.key}`,
      () => {},
    );
  }
  clearExchangeReferences(runtime, state);
  runtime.exchangeCompletedKey = state.key;
  runtime.exchange = null;
  runtime.exchangeSnapshot = null;
  runtime.exchangeSnapshotKey = null;
  runtime.onExchangeDone?.();
}

/**
 * 中途被打断（换局、阶段跳过、组件销毁）：把手牌层强制重建，把被演出挪动过的
 * 牌恢复成视图该有的样子。阶段推进挂在 rAF 上，没有要收的定时器。
 */
export function cleanupExchange(runtime: TableRuntime): void {
  const state = runtime.exchange;
  if (state) {
    /*
     * 换局/重连可能在四段演出中途触发。此前这里只清了引用和 layer signature，
     * 但飞出的三张仍挂在场景树上，于是第二局开局会看见上一局的残牌。把本次
     * 演出登记过的节点全部摘掉，并取消它们的动画；下一次 renderTable 会依据
     * 当前视图重建各家手牌。
     */
    const exchangeTiles = new Set<THREE.Group>(state.selfTiles.values());
    for (const stack of state.stacks.values()) {
      for (const group of stack.tiles) exchangeTiles.add(group);
    }
    runtime.animations = runtime.animations.filter(
      (animation) => !exchangeTiles.has(animation.group),
    );
    for (const group of exchangeTiles) {
      group.removeFromParent();
      group.visible = false;
    }
    clearExchangeReferences(runtime, state);
    for (const layerKey of [...runtime.layers.keys()]) {
      if (!layerKey.startsWith("hand:")) continue;
      const layer = runtime.layers.get(layerKey);
      if (layer) layer.signature = "";
    }
    runtime.exchange = null;
  }
  runtime.exchangeSnapshot = null;
  runtime.exchangeSnapshotKey = null;
}

function clearExchangeReferences(
  runtime: TableRuntime,
  state: ExchangeState,
): void {
  if (runtime.hovered) {
    for (const group of state.selfTiles.values()) {
      if (runtime.hovered === group) {
        runtime.hovered = null;
        break;
      }
    }
  }
}

/** 别家手牌层里那排可复用的牌背节点。 */
function opponentHandPool(runtime: TableRuntime, seat: number): THREE.Group[] {
  const layer = runtime.layers.get(`hand:${seat}`);
  if (!layer) return [];
  return layer.group.children.filter(
    (object): object is THREE.Group =>
      object instanceof THREE.Group &&
      object.userData.opponentHandPool === true &&
      object.visible,
  );
}

function byOpponentHandIndex(left: THREE.Group, right: THREE.Group): number {
  return (
    (left.userData.opponentHandTileIndex as number) -
    (right.userData.opponentHandTileIndex as number)
  );
}

/** 牌河中央第 `index` 张换牌摞的世界落点（平躺高度与牌河一致）。 */
function exchangeStackSlot(
  runtime: TableRuntime,
  relative: number,
  index: number,
  face = false,
): THREE.Vector3 {
  const position = exchangeStackPosition(
    relative,
    index,
    runtime.tileWidthRatio,
    runtime.tileScale,
  );
  /* 正面牌坯翻成牌背朝上时绕 X 轴转了 π；其 pivot 在牌底，必须把落点抬一
     个牌厚，否则主视角这摞牌会有半张陷进桌面。别家牌背不需要这份补偿。 */
  if (face) {
    position.y += TILE_LENGTH * TILE_DEPTH_RATIO * runtime.tileScale;
  }
  return position;
}

/**
 * 把 pivot 上的立姿/躺姿转角并进 group 的四元数。
 *
 * 飞行动画只驱动 group 一层，转角留在子节点上就转不起来；并进去之后
 * pivot 归零，起飞和落地的姿态都从四元数里读写。
 */
function bakePivotRotation(group: THREE.Group): THREE.Quaternion {
  const pivot = group.userData.tilePivot as THREE.Group;
  const pitch = new THREE.Quaternion().setFromAxisAngle(X_AXIS, pivot.rotation.x);
  const baked = group.quaternion.clone().multiply(pitch);
  pivot.rotation.x = 0;
  group.quaternion.copy(baked);
  return baked;
}

/** 平躺盖牌的四元数：座向跟着相对方位，牌面扣向桌面。 */
function flatQuaternion(relative: number, backMesh: boolean): THREE.Quaternion {
  return new THREE.Quaternion()
    .setFromAxisAngle(Y_AXIS, relative * (Math.PI / 2))
    .multiply(
      new THREE.Quaternion().setFromAxisAngle(X_AXIS, coveredHandTilt(backMesh)),
    );
}

/** 立牌的四元数：座向跟着相对方位，牌立在那家的手牌基线上。 */
function standingQuaternion(
  relative: number,
  backMesh: boolean,
): THREE.Quaternion {
  return new THREE.Quaternion()
    .setFromAxisAngle(Y_AXIS, relative * (Math.PI / 2))
    .multiply(
      new THREE.Quaternion().setFromAxisAngle(
        X_AXIS,
        standingHandTilt(backMesh),
      ),
    );
}
