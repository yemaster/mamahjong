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
  nukiRiverPosition,
  rotateAroundTable,
  tableRelativeSeat,
} from "./geometry";
import { riverDiscardEntries, sortHandForDisplay } from "./handView";
import { registerTableTile } from "./tileHighlight";
import { dimTile, makeTile, markTileAsDora, markTileAsWinning, rootTile } from "./tileMesh";
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
  riverDiscardEntries(player.discards).forEach((entry, index) => {
    addDiscardTile(
      runtime,
      view,
      player,
      previousPlayer,
      openingPhase,
      entry.discard,
      entry.originalIndex,
      entry.sideways,
      index,
    );
  });
  for (const [index, tile] of (player.nuki_tiles ?? []).entries()) {
    addNukiTile(runtime, view, player, tile, index);
  }
}

/** 牌河里的单张牌；正常出牌只新增这一层，不再重建前面已经落地的牌。 */
export function addDiscardTile(
  runtime: TableRuntime,
  view: MatchView,
  player: MatchPlayerView,
  previousPlayer: MatchPlayerView | undefined,
  openingPhase: OpeningPhase,
  discard: MatchPlayerView["discards"][number],
  originalIndex: number,
  sideways: boolean,
  index: number,
  sichuanRonInfo: readonly { payerSeat: number; tileId: number }[] = [],
): void {
  const now = performance.now();
  const relative = tableRelativeSeat(
    player.seat,
    view.observer_seat,
    view.players.length,
  );
  const isNewDiscard =
    openingPhase === "play" &&
    previousPlayer != null &&
    originalIndex >= previousPlayer.discards.length;
  const flightKey = `${player.seat}:${discard.tile.id}`;
  const riverPosition = discardGridPosition(
    index,
    runtime.tileWidthRatio,
    runtime.tileScale,
    3,
  );
  const group = makeTile(runtime, discard.tile.code, RIVER_TILE_LENGTH);
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
  /* 四川麻将荣和：放炮家牌河里被胡的那张染浅红。 */
  if (sichuanRonInfo.some((info) => info.payerSeat === player.seat && info.tileId === discard.tile.id)) {
    markTileAsWinning(group);
  }
  if (runtime.dimTsumogiri && discard.tsumogiri) {
    dimTile(group, TSUMOGIRI_DIM);
  }
  registerTableTile(runtime, group, discard.tile.code);
  rootTile(runtime, group);

  if (isNewDiscard) {
    const source = discardSource(
      runtime,
      view,
      previousPlayer,
      discard.tile.id,
      discard.tsumogiri,
      runtime.tileWidthRatio,
      runtime.tileScale,
    );
    runtime.discardFlights.set(flightKey, {
      startedAt: now,
      start: source,
      startRotation: handQuaternion(relative, relative === 0),
    });
  }

  const flight = runtime.discardFlights.get(flightKey);
  if (flight && now - flight.startedAt < DISCARD_FLIGHT_MS) {
    const destination = group.position.clone();
    const endRotation = group.quaternion.clone();
    group.position.copy(flight.start);
    group.quaternion.copy(flight.startRotation);
    runtime.animations.push({
      group,
      start: flight.start,
      end: destination,
      startRotation: flight.startRotation,
      endRotation,
      startedAt: flight.startedAt,
      duration: DISCARD_FLIGHT_MS,
    });
  } else if (flight) {
    runtime.discardFlights.delete(flightKey);
  }
}

/** 三麻拔北也按单张缓存，新增一枚北不碰已经摆好的北。 */
export function addNukiTile(
  runtime: TableRuntime,
  view: MatchView,
  player: MatchPlayerView,
  tile: MatchPlayerView["nuki_tiles"][number],
  index: number,
): void {
  const relative = tableRelativeSeat(
    player.seat,
    view.observer_seat,
    view.players.length,
  );
  const position = nukiRiverPosition(
    index,
    runtime.tileWidthRatio,
    runtime.tileScale,
  );
  const group = makeTile(runtime, tile.code, RIVER_TILE_LENGTH);
  const local = new THREE.Vector3(
    position.x,
    (RIVER_TILE_DEPTH * runtime.tileScale) / 2 + 0.09,
    position.z,
  );
  rotateAroundTable(local, relative);
  group.position.copy(local);
  group.rotation.y = relative * (Math.PI / 2);
  if (isDoraTile(tile.code, view.dora_indicators ?? [])) {
    markTileAsDora(runtime, group);
  }
  registerTableTile(runtime, group, tile.code);
  rootTile(runtime, group);
}

/**
 * 最后一张出牌的箭头是独立小层，不再寄生在整条牌河里。
 * 轮到下一家出牌时只替换这个箭头，上一家的所有牌面节点保持原样。
 */
export function addLastDiscardMarkerLayer(
  runtime: TableRuntime,
  view: MatchView,
  openingPhase: OpeningPhase,
): void {
  if (openingPhase !== "play" || !runtime.lastDiscard) return;
  const { seat, index: originalIndex } = runtime.lastDiscard;
  const player = view.players.find((candidate) => candidate.seat === seat);
  if (!player) return;
  const riverEntries = riverDiscardEntries(player.discards);
  const riverIndex = riverEntries.findIndex(
    (entry) => entry.originalIndex === originalIndex,
  );
  if (riverIndex < 0) return;
  const discard = player.discards[originalIndex];
  if (!discard) return;

  const relative = tableRelativeSeat(
    seat,
    view.observer_seat,
    view.players.length,
  );
  const riverPosition = discardGridPosition(
    riverIndex,
    runtime.tileWidthRatio,
    runtime.tileScale,
    3,
  );
  const anchor = new THREE.Group();
  const local = new THREE.Vector3(
    riverPosition.x,
    (RIVER_TILE_DEPTH * runtime.tileScale) / 2 + 0.09,
    riverPosition.z,
  );
  rotateAroundTable(local, relative);
  anchor.position.copy(local);
  anchor.scale.setScalar(runtime.tileScale);
  anchor.rotation.y =
    relative * (Math.PI / 2) +
    (riverEntries[riverIndex]?.sideways ? Math.PI / 2 : 0) +
    discardNaturalRotation(seat, originalIndex);

  const flight = runtime.discardFlights.get(`${seat}:${discard.tile.id}`);
  addLastDiscardMarker(
    runtime,
    anchor,
    flight ? flight.startedAt + DISCARD_FLIGHT_MS : performance.now(),
  );
  runtime.renderTarget.add(anchor);
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
  anchor: THREE.Group,
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
  anchor.add(marker);
  runtime.spinners.push(marker);
}
