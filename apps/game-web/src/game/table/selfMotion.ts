import * as THREE from "three";
import { clientRectInViewport } from "../../components/viewportCoordinates";
import type { MatchPlayerView, MatchView } from "../../types";
import type { OpeningPhase } from "../OpeningSequence";
import { billboardHandTilt, standingHandTilt } from "./animation";
import {
  TILE_DEPTH_RATIO,
  TILE_LENGTH,
  TILE_STAND_UP_MS,
} from "./constants";
import { handPosition, handQuaternion, screenRectAnchor } from "./geometry";
import { sortHandForDisplay } from "./handView";
import { DRAW_MOVE_MS, standUpOnArrival } from "./hands";
import { isDoraTile } from "../tileAssets";
import { isJokerTile } from "./handView";
import { makeTile, markTileAsDora, rootTile, tileBody } from "./tileMesh";
import type { TableRuntime } from "./types";
import type { WallLayout } from "./wallLayout";

/**
 * 主视角自己摸的那张牌。
 *
 * 自家的手牌平时由二维层画，桌上没有实体，所以摸牌时牌山上那张牌是直接消失的，
 * 手上凭空多出一张。这里给它补上中间那一段：一张真牌体从牌山飞出来，落到二维
 * 手牌里空着的那一格上翻起来立住，立住之后就地拆掉，交给二维手牌接手。
 *
 * 落点是**量二维那一格量出来的**，不是三维手牌那条基线：那条基线是照着别家的
 * 牌排的，比二维手牌窄一号也低一截，牌照它落地就会先缩在手牌左上方一点，再突
 * 然涨成二维那张——「摸牌不连贯」就是这么来的。量出来之后位置、大小、牌面朝向
 * 三样都对上，两层交接那一下就看不出来了。
 *
 * 它不属于任何一家的手牌，只在桌上待这一会儿，所以走 `transients` 而不是像其他
 * 三家那样每次重建都照着视图重新摆一遍。
 */
export function addSelfDraw(
  runtime: TableRuntime,
  view: MatchView,
  player: MatchPlayerView,
  previousPlayer: MatchPlayerView | undefined,
  openingPhase: OpeningPhase,
  wall: WallLayout,
  consumedTileCount: number,
  rinshanDrawNumber: number | null,
): void {
  if (openingPhase !== "play" || view.phase.kind === "ended") return;
  /*
   * 牌谱重演不飞这一段。那边一步就是一个状态，下一步一到整张桌子推倒重来，
   * 飞到半路的牌连同 `transients` 一起没了——牌在手边闪一下就不见，比不飞还难看。
   */
  if (runtime.instantDraw) return;
  const drawnTileId = player.drawn_tile_id;
  if (drawnTileId == null) return;
  /*
   * 换局那一帧不飞。新一局的视图先到，`openingPhase` 要等外面那个 effect 跑完才
   * 退回 `dice`，中间夹着整整一帧：手上已经是新一局的牌，阶段却还停在上一局的
   * `play`。照这一帧飞的话，牌山上会窜出一张莫名其妙的牌，下一帧随重建一起没。
   * 新一局的第一张牌本来就由开局发牌动画交代，这里不用管。
   */
  if (runtime.previousView != null &&
      runtime.previousView.hand_index !== view.hand_index) {
    return;
  }
  /*
   * 飞到半路撞上一次重建：牌跟着 `transients` 一起没了，二维那一格却还空着。
   * 认出是同一张、还在飞行时限内，就照原来的起飞时刻把它接着飞完。
   */
  const now = performance.now();
  const inFlight = runtime.selfDraw;
  const resuming =
    inFlight != null &&
    inFlight.tileId === drawnTileId &&
    now < inFlight.takeOffAt + DRAW_MOVE_MS + TILE_STAND_UP_MS;
  /* 同一张牌只飞一次：轮询刷新拿到的还是这张，就当它已经在手上了。 */
  if (
    !resuming &&
    rinshanDrawNumber == null &&
    (previousPlayer == null || previousPlayer.drawn_tile_id === drawnTileId)
  ) {
    return;
  }
  const tiles = sortHandForDisplay(
    player.concealed_tiles ?? [],
    drawnTileId,
    view.joker_code,
  );
  const index = tiles.findIndex((tile) => tile.id === drawnTileId);
  if (index < 0) return;

  /* 量不到二维那一格时的退路：照旧落在三维手牌基线上。 */
  const baseline = handPosition(
    0,
    tiles.length,
    index,
    true,
    /* 摸到的这张和原手牌之间留一道缝，跟二维手牌一致。 */
    0.2,
    false,
    runtime.tileWidthRatio,
    runtime.tileScale,
  );
  const slot = handSlotAnchor(runtime, drawnTileId, baseline);
  const destination = slot?.position ?? baseline;
  const endTilt = slot?.tilt ?? standingHandTilt(false);
  const endScale = slot?.scale ?? runtime.tileScale;
  const wallSlot =
    resuming && inFlight
      ? inFlight.wallSlot
      : rinshanDrawNumber != null
        ? wall.rinshanSlot(rinshanDrawNumber)
        : wall.drawSlot(Math.max(0, consumedTileCount - 1));
  const start = wall.origin(wallSlot, runtime.tileWidthRatio, runtime.tileScale);
  const startRotation = wall.quaternion(wallSlot);
  const endRotation = handQuaternion(0, true);

  const group = makeTile(runtime, tiles[index]!.code, TILE_LENGTH);
  const facePlate = group.userData.facePlateMaterial as
    | THREE.MeshBasicMaterial
    | undefined;
  facePlate?.color.set(0xefefef);
  if (
    isDoraTile(tiles[index]!.code, view.dora_indicators ?? []) ||
    isJokerTile(tiles[index]!.code, view.joker_code)
  ) {
    markTileAsDora(runtime, group);
  }
  tileBody(group).rotation.x = 0;
  group.position.copy(start);
  group.quaternion.copy(startRotation);
  rootTile(runtime, group);

  const takeOffAt = resuming && inFlight ? inFlight.takeOffAt : now;
  runtime.selfDraw = { tileId: drawnTileId, takeOffAt, wallSlot };
  runtime.animations.push({
    group,
    start,
    end: destination,
    startRotation,
    endRotation,
    startedAt: takeOffAt,
    duration: DRAW_MOVE_MS,
    startScale: runtime.tileScale,
    endScale,
  });
  standUpOnArrival(
    runtime,
    group,
    group.userData.tilePivot as THREE.Group,
    destination,
    false,
    endTilt,
    takeOffAt + DRAW_MOVE_MS,
  );
  runtime.transients.push({
    group,
    /*
     * 对齐了就多留一手：二维那一格什么时候补上由 React 那边的定时器说了算，两
     * 边差一帧就会露出一个空当。牌既然停在同一个地方，多待这一会儿是叠着的，看
     * 不出来；没对齐（量不到那一格）就别赖着，两张错位的牌同时在反而更扎眼。
     */
    removeAt:
      takeOffAt +
      DRAW_MOVE_MS +
      TILE_STAND_UP_MS +
      (slot ? SELF_DRAW_HANDOFF_MS : 0),
  });
}

/** 三维飞牌落地后多停一会儿再拆，给二维手牌补格子留出余量。 */
const SELF_DRAW_HANDOFF_MS = 90;

/**
 * 二维手牌里给这张牌留的那一格，换算成三维里的落点、缩放和牌面朝向。
 *
 * 量的是真元素而不是照着 CSS 再算一遍：那排牌的大小还要过牌桌设置里的手牌缩放
 * 和舞台缩放两道，只有量出来的才和玩家眼前那一格分毫不差。牌谱预览之类没有二维
 * 手牌的场合量不到，返回 `null`，由调用方退回三维手牌基线。
 *
 * 这一格是空着占位的（`is-landing`），不是被摘掉了——摘掉就没处可量。
 */
function handSlotAnchor(
  runtime: TableRuntime,
  tileId: number,
  depthReference: THREE.Vector3,
): { position: THREE.Vector3; scale: number; tilt: number } | null {
  if (typeof document === "undefined") return null;
  const canvas = runtime.renderer.domElement;
  const scope = canvas.closest(".match-screen") ?? document;
  const slot = scope.querySelector(
    `.match-hand-2d__tile[data-hand-tile-id="${tileId}"]`,
  );
  if (!slot) return null;
  const slotRect = clientRectInViewport(slot);
  const canvasRect = clientRectInViewport(canvas);
  if (
    slotRect.width <= 0 ||
    canvasRect.width <= 0 ||
    canvasRect.height <= 0
  ) {
    return null;
  }
  const anchor = screenRectAnchor(
    runtime.camera,
    { width: canvasRect.width, height: canvasRect.height },
    {
      centerX: slotRect.left + slotRect.width / 2 - canvasRect.left,
      centerY: slotRect.top + slotRect.height / 2 - canvasRect.top,
      width: slotRect.width,
    },
    depthReference,
    TILE_LENGTH * runtime.tileWidthRatio,
  );
  const tilt = billboardHandTilt(runtime.camera);
  /*
   * 对上的得是牌面，不是 group 原点：牌体是从原点沿着牌面法线叠上去的（半个厚
   * 度到牌体中心，再半个到牌面），这一段先减掉，牌面才正好停在那一格上。
   */
  const faceOffset = new THREE.Vector3(
    0,
    Math.cos(tilt),
    Math.sin(tilt),
  ).multiplyScalar(TILE_LENGTH * TILE_DEPTH_RATIO * anchor.scale);
  return {
    position: anchor.position.sub(faceOffset),
    scale: anchor.scale,
    tilt,
  };
}
