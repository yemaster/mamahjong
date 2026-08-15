import * as THREE from "three";
import type { MatchPlayerView, MatchView } from "../../types";
import type { OpeningPhase } from "../OpeningSequence";
import { isDoraTile } from "../tileAssets";
import { isJokerTile } from "./handView";
import {
  coveredHandTilt,
  openingDealOrder,
  openingDealStep,
  SETTLEMENT_COVER_MS,
  SETTLEMENT_REVEAL_MS,
  settlementHandShift,
  settlementHandTilt,
  standUpEase,
  standingHandTilt,
  TSUMO_THROW_ARC,
  TSUMO_THROW_MS,
  tsumoThrowEase,
} from "./animation";
import {
  OPENING_DEAL_MOVE_MS,
  OPENING_DEAL_STEP_MS,
  OPPONENT_TILE_LENGTH,
  TILE_DEPTH_RATIO,
  TILE_LENGTH,
  TILE_STAND_UP_MS,
} from "./constants";
import {
  handPosition,
  handQuaternion,
  tableRelativeSeat,
} from "./geometry";
import {
  canLocalPlayerDiscard,
  playerIsHoldingDrawnTile,
  sortHandForDisplay,
  winningTileIndex,
} from "./handView";
import { IMPACT_DUST_SPREAD, spawnTableImpact } from "./impact";
import {
  makeTile,
  markTileAsDora,
  rootTile,
  tileBody,
  tileFaceMesh,
} from "./tileMesh";
import type { TableRuntime } from "./types";
import type { WallLayout } from "./wallLayout";
import { opponentHandLayout } from "./opponentHandMotion";

/**
 * 一家的手牌。
 *
 * 平时是一排立着的牌；开局从牌墙飞过来，摸牌单独飞一张；结算时整把牌一起
 * 倒下（摊牌）或翻扣（盖牌）。自家的手牌在对局中由 2D 层画，这里只在终局
 * 之后才补上。
 */
export function addHand(
  runtime: TableRuntime,
  view: MatchView,
  player: MatchPlayerView,
  previousPlayer: MatchPlayerView | undefined,
  openingPhase: OpeningPhase,
  settlementRevealSeats: number[],
  settlementWinningTileSeats: number[],
  wall: WallLayout,
  consumedTileCount: number,
  rinshanDrawNumber: number | null,
): void {
  const relative = tableRelativeSeat(
    player.seat,
    view.observer_seat,
    view.players.length,
  );
  const isSelf = relative === 0;
  const settling = view.hand_settlement != null;
  const revealed = settlementRevealSeats.includes(player.seat);
  const winningTileShown = settlementWinningTileSeats.includes(player.seat);
  const spreadsOpen =
    settling &&
    (view.hand_settlement!.tenpai_seats.includes(player.seat) ||
      view.hand_settlement!.winners.some(
        (winner) => winner.seat === player.seat,
      ));
  if (isSelf && view.phase.kind !== "ended") {
    return;
  }
  const selfTilesKnown =
    isSelf && (player.concealed_tiles?.length ?? 0) > 0;
  /*
   * 牌谱重演的摊牌：另外三家的手牌正面朝上平摊在桌面上，和结算摊牌一个姿势——
   * 要的就是「看得见别人手里是什么」，立着的牌看不清。这条旁路只有牌谱走得到，
   * 而且拖进度条时是直接摆到位的，不重放一遍倒牌动画（见下面的 `alreadyFallen`）。
   */
  const revealAll = runtime.revealAllHands;
  const now = performance.now();
  const actuallyHolding = !isSelf && playerIsHoldingDrawnTile(view, player.seat);
  /*
   * 服务端的 concealed_tile_count 就是真实总数，任何动画阶段都不能改它。
   * 摸牌时最后一张只是被摆到独立槽位，绝不能为了“藏摸入牌”把数量减一。
   */
  const hiddenCount = player.concealed_tile_count;
  const hiddenTiles = Array.from(
    { length: hiddenCount },
    (_, index) => ({ id: -1 - index, code: "back" }),
  );
  const knownTiles = sortHandForDisplay(
    player.concealed_tiles ?? [],
    player.drawn_tile_id,
    view.joker_code,
  );
  /* A revealed hand still needs tiles on the felt: if the server has not sent
     the concealed tiles yet, keep the backs standing rather than blank out
     the whole hand. */
  const tiles =
    (selfTilesKnown ||
      revealAll ||
      (settling && (revealed || winningTileShown) && spreadsOpen)) &&
    knownTiles.length > 0
      ? knownTiles
      : hiddenTiles;
  /*
   * 对手暗手只做槽位变换：摸后 N 张仍画 N 张；手切后 N 张仍画 N 张，只额外
   * 留一个空槽。牌的真实数量在整段动画里始终不变。
   */
  const gap = !isSelf ? runtime.handCutGaps.get(player.seat) : undefined;
  const opponentLayout =
    !isSelf && tiles === hiddenTiles
      ? opponentHandLayout(
          hiddenTiles.length,
          actuallyHolding,
          gap?.gapPosition,
        )
      : null;
  const tilesWithGap = opponentLayout
    ? (Array.from(
        { length: opponentLayout.slotCount },
        () => null,
      ) as Array<(typeof hiddenTiles)[number] | null>)
    : null;
  opponentLayout?.renderedSlots.forEach((slot, tileIndex) => {
    tilesWithGap![slot] = hiddenTiles[tileIndex]!;
  });
  const displayTiles = tilesWithGap ?? tiles;
  /* Without the actual tiles there is nothing to show face up, so the hand
     turns over instead of spreading. */
  const faceUp = (spreadsOpen || revealAll) && tiles === knownTiles;
  const length = isSelf ? TILE_LENGTH : OPPONENT_TILE_LENGTH;
  /* 自摸 flips the drawn tile up on its own before the hand falls open. */
  const winningIndex =
    settling && faceUp ? winningTileIndex(tiles, player.drawn_tile_id) : -1;
  /*
   * 甩牌只属于自摸：牌是自己摸上来的，摔在桌上才有那股劲。荣和的和了牌是别人
   * 打出来的，流局摊的听牌更没什么可甩，这两种都只是把手牌摊开。
   */
  const tsumoWin =
    settling &&
    view.hand_settlement!.reason === "tsumo" &&
    view.hand_settlement!.winners.some(
      (winner) => winner.seat === player.seat,
    );
  /* 整把牌一起倒，不做逐张的涟漪，砸下去更整齐利落。 */
  const fallStartedAt = performance.now();

  displayTiles.forEach((tile, index) => {
    if (tile === null) return; // 手切空隙——不渲染
    const isDrawn =
      (isSelf && tile.id === player.drawn_tile_id) ||
      (!isSelf && opponentLayout?.drawnSlot === index);
    const drawnGap = isDrawn ? 0.2 : 0;
    const isWinningTile = index === winningIndex;
    /* 自摸的那张牌是从高处砸到桌上的，落点和其余手牌同一条线。 */
    const thrown = isWinningTile && faceUp && tsumoWin;
    /* The winning tile leads the reveal, the rest of the hand follows. */
    const tileRevealed =
      revealAll || (settling && (revealed || (winningTileShown && isWinningTile)));
    const tileFaceUp = tileRevealed && faceUp;
    const meshCode = selfTilesKnown || tileFaceUp ? tile.code : "back";
    const backMesh = meshCode === "back";
    const group = makeTile(runtime, meshCode, length);
    if (
      (isSelf || tileFaceUp) &&
      (isDoraTile(tile.code, view.dora_indicators ?? []) ||
        isJokerTile(tile.code, view.joker_code))
    ) {
      markTileAsDora(runtime, group);
    }
    const worldLength = length * runtime.tileScale;
    const standingPosition = handPosition(
      relative,
      displayTiles.length,
      index,
      isSelf,
      drawnGap,
      false,
      runtime.tileWidthRatio,
      runtime.tileScale,
    );
    const settledPosition = handPosition(
      relative,
      tiles.length,
      index,
      isSelf,
      drawnGap,
      true,
      runtime.tileWidthRatio,
      runtime.tileScale,
      /* 自摸那张牌也用同一个位移：砸下来之后要和其余的牌摊在同一条线上。 */
      settlementHandShift(
        faceUp,
        worldLength,
        worldLength * TILE_DEPTH_RATIO,
      ),
    );
    group.rotation.y = relative * (Math.PI / 2);
    const handBody = tileBody(group);
    const handPivot = group.userData.tilePivot as THREE.Group;
    const standingTilt = standingHandTilt(backMesh);
    const settlementTilt = settlementHandTilt(faceUp, backMesh);
    handBody.rotation.x = 0;
    const alreadyFallen =
      revealAll ||
      runtime.revealedSettlementSeats.has(player.seat) ||
      (isWinningTile && runtime.revealedWinningTileSeats.has(player.seat));
    if (tileRevealed && alreadyFallen) {
      group.position.copy(settledPosition);
      handPivot.rotation.x = settlementTilt;
    } else if (tileRevealed) {
      group.position.copy(standingPosition);
      handPivot.rotation.x = standingTilt;
      runtime.tilts.push({
        object: handPivot,
        group,
        startX: standingTilt,
        endX: settlementTilt,
        startPosition: standingPosition.clone(),
        endPosition: settledPosition.clone(),
        startedAt: fallStartedAt,
        duration: thrown
          ? TSUMO_THROW_MS
          : faceUp
            ? SETTLEMENT_REVEAL_MS
            : SETTLEMENT_COVER_MS,
        covering: !faceUp,
        arcHeight: thrown ? TSUMO_THROW_ARC : undefined,
        /* 摊平和位移跟着下落走，贴到桌面那一下正好到位。 */
        ease: thrown ? tsumoThrowEase : undefined,
      });
      if (thrown) {
        /* 落地那一刻从牌底下扬起一层灰，同时把镜头撞一下：牌自己不弹，
           冲击感全交给这两样。 */
        const impactAt = settledPosition.clone();
        impactAt.y -= (worldLength * TILE_DEPTH_RATIO) / 2;
        impactAt.y += 0.004;
        spawnTableImpact(
          runtime,
          impactAt,
          worldLength * IMPACT_DUST_SPREAD,
          fallStartedAt + TSUMO_THROW_MS,
        );
      }
    } else {
      group.position.copy(standingPosition);
      handPivot.rotation.x = standingTilt;
    }

    group.userData.baseY = group.position.y;
    if (!isSelf && tiles === hiddenTiles) {
      /* 供手切空隙结束后原地归拢；索引对应紧凑牌阵，不受临时空槽影响。 */
      group.userData.opponentHandPool = true;
      group.userData.opponentHandTileIndex = -1 - tile.id;
    }

    const canDiscard =
      isSelf &&
      openingPhase === "play" &&
      canLocalPlayerDiscard(view);
    if (canDiscard) {
      group.userData.tileId = tile.id;
      const faceMesh = tileFaceMesh(group);
      faceMesh.userData.tileGroup = group;
      runtime.selectable.push(faceMesh);
    }
    rootTile(runtime, group);

    if (openingPhase === "deal") {
      const destination = group.position.clone();
      const dealOrder = openingDealOrder(
        index,
        player.seat,
        view.progress.dealer,
        view.players.length,
      );
      const wallSlot = wall.drawSlot(dealOrder);
      const start = wall.origin(
        wallSlot,
        runtime.tileWidthRatio,
        runtime.tileScale,
      );
      const startRotation = wall.quaternion(wallSlot);
      const endRotation = handQuaternion(relative, isSelf);
      handBody.rotation.x = 0;
      group.position.copy(start);
      group.quaternion.copy(startRotation);
      const takeOffAt =
        performance.now() +
        openingDealStep(
          index,
          player.seat,
          view.progress.dealer,
          view.players.length,
        ) *
          OPENING_DEAL_STEP_MS;
      runtime.animations.push({
        group,
        start,
        end: destination,
        startRotation,
        endRotation,
        startedAt: takeOffAt,
        duration: OPENING_DEAL_MOVE_MS,
        arcHeight: 0.9,
      });
      standUpOnArrival(
        runtime,
        group,
        handPivot,
        destination,
        backMesh,
        standingTilt,
        takeOffAt + OPENING_DEAL_MOVE_MS,
      );
    } else if (
      openingPhase === "play" &&
      isDrawn &&
      (rinshanDrawNumber != null ||
        (isSelf
          ? tile.id !== previousPlayer?.drawn_tile_id
          : previousPlayer != null &&
            player.concealed_tile_count >
              previousPlayer.concealed_tile_count))
    ) {
      const destination = group.position.clone();
      const wallSlot = rinshanDrawNumber != null
        ? wall.rinshanSlot(rinshanDrawNumber)
        : wall.drawSlot(Math.max(0, consumedTileCount - 1));
      const start = wall.origin(
        wallSlot,
        runtime.tileWidthRatio,
        runtime.tileScale,
      );
      const startRotation = wall.quaternion(wallSlot);
      const endRotation = handQuaternion(relative, isSelf);
      handBody.rotation.x = 0;
      group.position.copy(start);
      group.quaternion.copy(startRotation);
      const takeOffAt = now;
      runtime.animations.push({
        group,
        start,
        end: destination,
        startRotation,
        endRotation,
        startedAt: takeOffAt,
        duration: DRAW_MOVE_MS,
      });
      standUpOnArrival(
        runtime,
        group,
        handPivot,
        destination,
        backMesh,
        standingTilt,
        takeOffAt + DRAW_MOVE_MS,
      );
    }
  });
}

/**
 * 实时牌局中的对手暗手全是同一种牌背，不需要每次摸打都销毁整排再造。
 *
 * 这里把现有节点当作一个小对象池：摸牌只唤醒（首次才创建）一张，打牌/副露只把
 * 多出来的节点藏回池里，其余牌只改位置。结算、明牌和牌谱摊牌仍交给 `addHand`，
 * 因为那些状态确实会改变每一张牌的正反面和下落动画。
 */
export function syncHiddenOpponentHand(
  runtime: TableRuntime,
  view: MatchView,
  player: MatchPlayerView,
  previousPlayer: MatchPlayerView | undefined,
  openingPhase: OpeningPhase,
  wall: WallLayout,
  consumedTileCount: number,
  rinshanDrawNumber: number | null,
): boolean {
  const relative = tableRelativeSeat(
    player.seat,
    view.observer_seat,
    view.players.length,
  );
  const layer = runtime.layers.get(`hand:${player.seat}`);
  if (
    relative === 0 ||
    !layer ||
    !previousPlayer ||
    openingPhase !== "play" ||
    view.hand_settlement != null ||
    runtime.revealAllHands
  ) {
    return false;
  }

  const pooled = layer.group.children.filter(
    (object): object is THREE.Group =>
      object instanceof THREE.Group && object.userData.opponentHandPool === true,
  );
  if (pooled.length === 0) return false;
  /* 显示设置改变时应正常重建这一层，不能拿旧尺寸的牌硬挪。 */
  const sample = pooled[0]!;
  const sampleLength = sample.userData.tileLength as number | undefined;
  const sampleWidth = sample.userData.tileWidth as number | undefined;
  if (
    sampleLength == null ||
    sampleWidth == null ||
    Math.abs(sample.scale.x - runtime.tileScale) > 1e-6 ||
    Math.abs(sampleWidth / sampleLength - runtime.tileWidthRatio) > 1e-6
  ) {
    return false;
  }

  const desiredCount = Math.max(0, player.concealed_tile_count);
  let active = pooled
    .filter((group) => group.visible)
    .sort(
      (left, right) =>
        (left.userData.opponentHandTileIndex as number) -
        (right.userData.opponentHandTileIndex as number),
    );
  const gap = runtime.handCutGaps.get(player.seat)?.gapPosition;

  while (active.length > desiredCount) {
    const removeAt =
      gap != null && active.length === previousPlayer.concealed_tile_count
        ? Math.min(active.length - 1, Math.max(0, gap))
        : active.length - 1;
    const [removed] = active.splice(removeAt, 1);
    if (!removed) break;
    removed.visible = false;
    delete removed.userData.opponentHandTileIndex;
    runtime.animations = runtime.animations.filter(
      (animation) => animation.group !== removed,
    );
    runtime.tilts = runtime.tilts.filter((tilt) => tilt.group !== removed);
  }

  const created: THREE.Group[] = [];
  while (active.length < desiredCount) {
    let group = pooled.find(
      (candidate) => !candidate.visible && !active.includes(candidate),
    );
    if (!group) {
      group = makeTile(runtime, "back", OPPONENT_TILE_LENGTH);
      group.userData.opponentHandPool = true;
      layer.group.add(group);
      pooled.push(group);
    }
    group.visible = true;
    active.push(group);
    created.push(group);
  }

  const holdingDrawn = playerIsHoldingDrawnTile(view, player.seat);
  const layout = opponentHandLayout(desiredCount, holdingDrawn, gap);
  const now = performance.now();
  for (const [index, group] of active.entries()) {
    const slot = layout.renderedSlots[index] ?? index;
    const isDrawn = layout.drawnSlot === slot;
    const destination = handPosition(
      relative,
      layout.slotCount,
      slot,
      false,
      isDrawn ? 0.2 : 0,
      false,
      runtime.tileWidthRatio,
      runtime.tileScale,
    );
    group.userData.opponentHandTileIndex = index;
    group.userData.baseY = destination.y;

    if (!created.includes(group)) {
      group.position.copy(destination);
      group.quaternion.copy(handQuaternion(relative, false));
      continue;
    }

    const wallSlot =
      rinshanDrawNumber != null
        ? wall.rinshanSlot(rinshanDrawNumber)
        : wall.drawSlot(Math.max(0, consumedTileCount - 1));
    const start = wall.origin(
      wallSlot,
      runtime.tileWidthRatio,
      runtime.tileScale,
    );
    const startRotation = wall.quaternion(wallSlot);
    const endRotation = handQuaternion(relative, false);
    const handPivot = group.userData.tilePivot as THREE.Group;
    const handBody = tileBody(group);
    const standingTilt = standingHandTilt(true);
    handBody.rotation.x = 0;
    group.position.copy(start);
    group.quaternion.copy(startRotation);
    runtime.animations.push({
      group,
      start,
      end: destination,
      startRotation,
      endRotation,
      startedAt: now,
      duration: DRAW_MOVE_MS,
    });
    standUpOnArrival(
      runtime,
      group,
      handPivot,
      destination,
      true,
      standingTilt,
      now + DRAW_MOVE_MS,
    );
  }
  return true;
}

/** 摸牌单飞一张的时长。 */
export const DRAW_MOVE_MS = 430;

/** 手切空位停留结束后，牌阵向左归拢的时长。 */
export const HAND_COLLAPSE_MS = 320;

/**
 * 牌在牌山里是盖着平躺的，飞到手上才翻起来立住。
 *
 * 起飞前先把它按平（和牌墙上的姿态一致），落位的那一刻再排一段翻牌。
 */
export function standUpOnArrival(
  runtime: TableRuntime,
  group: THREE.Group,
  handPivot: THREE.Group,
  destination: THREE.Vector3,
  backMesh: boolean,
  standingTilt: number,
  arrivalAt: number,
): void {
  const coveredTilt = coveredHandTilt(backMesh);
  handPivot.rotation.x = coveredTilt;
  runtime.tilts.push({
    object: handPivot,
    group,
    startX: coveredTilt,
    endX: standingTilt,
    startPosition: destination.clone(),
    endPosition: destination.clone(),
    startedAt: arrivalAt,
    duration: TILE_STAND_UP_MS,
    covering: false,
    ease: standUpEase,
  });
}
