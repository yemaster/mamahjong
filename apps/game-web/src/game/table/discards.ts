import * as THREE from "three";
import type { MatchPlayerView, MatchView } from "../../types";
import type { OpeningPhase } from "../OpeningSequence";
import { DISCARD_FLIGHT_MS } from "../animationTiming";
import { isDoraTile } from "../tileAssets";
import { isJokerTile } from "./handView";
import { RIVER_TILE_DEPTH, RIVER_TILE_LENGTH } from "./constants";
import {
  discardGridPosition,
  discardNaturalRotation,
  handPosition,
  handQuaternion,
  rotateAroundTable,
  tableRelativeSeat,
} from "./geometry";
import { riverDiscardEntries, sortHandForDisplay } from "./handView";
import { registerTableTile } from "./tileHighlight";
import { dimTile, makeTile, markTileAsDora, rootTile } from "./tileMesh";
import type { TableRuntime } from "./types";

/**
 * 摸切压到几成。
 *
 * 只压到看得出深浅，不压到认不出牌：牌河终究是要读牌面的，暗一档是提示，不是遮盖。
 */
const TSUMOGIRI_DIM = 0.68;

/** 一家的牌河：六张一行，立直的那张横过来，刚打出的那张从手边飞过去。 */
export function addDiscards(
  runtime: TableRuntime,
  view: MatchView,
  player: MatchPlayerView,
  previousPlayer: MatchPlayerView | undefined,
  openingPhase: OpeningPhase,
): void {
  const relative = tableRelativeSeat(
    player.seat,
    view.observer_seat,
    view.players.length,
  );
  riverDiscardEntries(player.discards).forEach(
    ({ discard, originalIndex, sideways }, index) => {
      const isNewDiscard =
        openingPhase === "play" &&
        previousPlayer &&
        originalIndex >= previousPlayer.discards.length;
      /* 冲击麻将摸牌多，牌河用四排；立直保持三排。 */
      const riverMaxRow = view.variant_kind === "impact" ? 3 : 2;
      const riverPosition = discardGridPosition(
        index,
        runtime.tileWidthRatio,
        runtime.tileScale,
        riverMaxRow,
      );
      const group = makeTile(
        runtime,
        discard.tile.code,
        RIVER_TILE_LENGTH,
      );
      const local = new THREE.Vector3(
        riverPosition.x,
        (RIVER_TILE_DEPTH * runtime.tileScale) / 2 + 0.09,
        riverPosition.z,
      );
      rotateAroundTable(local, relative);
      group.position.copy(local);
      group.rotation.y =
        relative * (Math.PI / 2) +
        (sideways ? Math.PI / 2 : 0) +
        discardNaturalRotation(player.seat, originalIndex);
      if (
        isDoraTile(discard.tile.code, view.dora_indicators ?? []) ||
        isJokerTile(discard.tile.code, view.joker_code)
      ) {
        markTileAsDora(runtime, group);
      }
      /*
       * 摸切压暗。压在 `registerTableTile` 之前：点亮同种牌那套记的是这一刻的颜色，
       * 先压暗，后面染蓝染红都是在暗底上乘一层，暗的还是暗的。
       */
      if (runtime.dimTsumogiri && discard.tsumogiri) {
        dimTile(group, TSUMOGIRI_DIM);
      }
      registerTableTile(runtime, group, discard.tile.code);
      rootTile(runtime, group);

      if (
        runtime.lastDiscard?.seat === player.seat &&
        runtime.lastDiscard.index === originalIndex
      ) {
        addLastDiscardMarker(
          runtime,
          group,
          isNewDiscard
            ? performance.now() + DISCARD_FLIGHT_MS
            : performance.now(),
        );
      }

      /* 四家都保留从手边飞入牌河的动画；只有空位与归拢逻辑限定为另外三家。 */
      if (isNewDiscard) {
        const destination = group.position.clone();
        const endRotation = group.quaternion.clone();
        const source = discardSource(
          runtime,
          view,
          previousPlayer,
          discard.tile.id,
          discard.tsumogiri,
          runtime.tileWidthRatio,
          runtime.tileScale,
        );
        group.position.copy(source);
        group.quaternion.copy(handQuaternion(relative, relative === 0));
        runtime.animations.push({
          group,
          start: source,
          end: destination,
          startRotation: group.quaternion.clone(),
          endRotation,
          startedAt: performance.now(),
          duration: DISCARD_FLIGHT_MS,
        });
      }
    },
  );
}

/** 打出的那张牌原本站在手牌的哪个位置。 */
function discardSource(
  runtime: TableRuntime,
  view: MatchView,
  previousPlayer: MatchPlayerView,
  tileId: number,
  tsumogiri: boolean,
  widthRatio: number,
  tileScale: number,
): THREE.Vector3 {
  const relative = tableRelativeSeat(
    previousPlayer.seat,
    view.observer_seat,
    view.players.length,
  );
  if (relative === 0) {
    const previousTiles = sortHandForDisplay(
      previousPlayer.concealed_tiles ?? [],
      previousPlayer.drawn_tile_id,
      view.joker_code,
    );
    const exactIndex = previousTiles.findIndex((tile) => tile.id === tileId);
    const index =
      exactIndex >= 0
        ? exactIndex
        : tsumogiri
          ? Math.max(0, previousTiles.length - 1)
          : 0;
    const drawnGap =
      previousTiles[index]?.id === previousPlayer.drawn_tile_id ? 0.2 : 0;
    return handPosition(
      relative,
      previousTiles.length,
      index,
      true,
      drawnGap,
      false,
      widthRatio,
      tileScale,
    );
  }

  const count = Math.max(1, previousPlayer.concealed_tile_count);
  const index = tsumogiri
    ? count - 1
    : (runtime.handCutGaps.get(previousPlayer.seat)?.gapPosition ?? 0);
  return handPosition(
    relative,
    count,
    index,
    false,
    tsumogiri ? 0.2 : 0,
    false,
    widthRatio,
    tileScale,
  );
}

/** 记住当前该被响应的那张牌，好在它头上插一个旋转的箭头。 */
export function updateLastDiscard(
  runtime: TableRuntime,
  view: MatchView,
  previousView: MatchView | null,
): void {
  if (previousView && previousView.hand_index !== view.hand_index) {
    runtime.lastDiscard = null;
  }
  for (const player of view.players) {
    const previousCount =
      previousView?.players.find((candidate) => candidate.seat === player.seat)
        ?.discards.length ?? player.discards.length;
    if (player.discards.length > previousCount) {
      runtime.lastDiscard = {
        seat: player.seat,
        index: player.discards.length - 1,
      };
      return;
    }
  }
  if (view.phase.kind === "awaiting_responses") {
    const triggerSeat = view.phase.trigger_seat;
    const player = view.players.find(
      (candidate) => candidate.seat === triggerSeat,
    );
    if (player && player.discards.length > 0) {
      runtime.lastDiscard = {
        seat: player.seat,
        index: player.discards.length - 1,
      };
    }
  }
}

function addLastDiscardMarker(
  runtime: TableRuntime,
  tileGroup: THREE.Group,
  appearAt: number,
): void {
  const marker = new THREE.Mesh(
    new THREE.ConeGeometry(0.1, 0.18, 4),
    new THREE.MeshStandardMaterial({
      color: 0xe7b955,
      emissive: 0x6b300d,
      emissiveIntensity: 0.52,
      roughness: 0.35,
      metalness: 0.32,
      transparent: true,
      opacity: 0.92,
    }),
  );
  marker.position.set(0, 0.62, -0.12);
  marker.rotation.y = Math.PI / 4;
  marker.castShadow = false;
  marker.userData.baseY = 0.62;
  marker.userData.appearAt = appearAt;
  tileGroup.add(marker);
  runtime.spinners.push(marker);
}
