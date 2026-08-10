import type * as THREE from "three";
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
  countCompletedKans,
  playerCompletedKan,
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

    /*
     * 空位停满一秒后归拢。每张牌从手切状态下自己的旧槽位滑到紧凑槽位；桌面即使
     * 在途中因版本刷新重建，startedAt 也不变，因此不会跳回起点或多播一次。
     */
    const collapse = !isSelf
      ? runtime.handCollapses.get(player.seat)
      : undefined;
    if (
      collapse &&
      tiles === hiddenTiles &&
      !gap &&
      now - collapse.startedAt < HAND_COLLAPSE_MS
    ) {
      const sourceLayout = opponentHandLayout(
        hiddenTiles.length,
        false,
        collapse.gapPosition,
      );
      const sourceSlot = sourceLayout.renderedSlots[index];
      if (sourceSlot != null) {
        const source = handPosition(
          relative,
          sourceLayout.slotCount,
          sourceSlot,
          false,
          sourceSlot === sourceLayout.drawnSlot ? 0.2 : 0,
          false,
          runtime.tileWidthRatio,
          runtime.tileScale,
        );
        const destination = group.position.clone();
        group.position.copy(source);
        runtime.animations.push({
          group,
          start: source,
          end: destination,
          startedAt: collapse.startedAt,
          duration: HAND_COLLAPSE_MS,
        });
      }
    } else if (collapse && now - collapse.startedAt >= HAND_COLLAPSE_MS) {
      runtime.handCollapses.delete(player.seat);
    }
    group.userData.baseY = group.position.y;

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
      (playerCompletedKan(player, previousPlayer) ||
        (isSelf
          ? tile.id !== previousPlayer?.drawn_tile_id
          : previousPlayer != null &&
            player.concealed_tile_count >
              previousPlayer.concealed_tile_count))
    ) {
      const destination = group.position.clone();
      const wallSlot = playerCompletedKan(player, previousPlayer)
        ? wall.rinshanSlot(countCompletedKans(view))
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
