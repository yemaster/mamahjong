import type * as THREE from "three";
import type { MatchPlayerView, MatchView } from "../../types";
import { MELD_PUSH_MS, meldPushSource } from "./animation";
import { MELD_GROUP_GAP, MELD_TILE_LENGTH } from "./constants";
import {
  addedKanTilePosition,
  meldTilePosition,
  tableRelativeSeat,
} from "./geometry";
import { meldDisplayTiles } from "./handView";
import { isDoraTile } from "../tileAssets";
import { isJokerTile } from "./handView";
import { registerTableTile } from "./tileHighlight";
import { makeTile, markTileAsDora, rootTile } from "./tileMesh";
import type { TableRuntime } from "./types";

/**
 * 一家的副露区，从右往左排。
 *
 * 刚成立的吃/碰/杠会整组从手牌边缘推到副露位；加杠只是往已经摆好的碰上再叠
 * 一张，桌上没有「推」这个动作，所以不给动画。
 */
export function addMelds(
  runtime: TableRuntime,
  view: MatchView,
  player: MatchPlayerView,
  previousPlayer: MatchPlayerView | undefined,
): void {
  const relative = tableRelativeSeat(
    player.seat,
    view.observer_seat,
    view.players.length,
  );
  const knownMeldIds = new Set(
    (previousPlayer?.melds ?? []).map((meld) => meld.id),
  );
  const pushStartedAt = performance.now();
  let cursor = 0;
  for (const meld of player.melds) {
    const pushing =
      previousPlayer != null &&
      !knownMeldIds.has(meld.id) &&
      meld.kind !== "added_kan";
    const sourceRelative =
      meld.called_from == null
        ? null
        : tableRelativeSeat(
            meld.called_from,
            player.seat,
            view.players.length,
          );
    const displayTiles = meldDisplayTiles(meld, sourceRelative);
    let calledTilePosition: THREE.Vector3 | null = null;
    displayTiles.forEach(
      ({ tile, calledRotation, addedBesideCalled, faceDown }) => {
      const rotated = calledRotation !== 0;
      const group = makeTile(
        runtime,
        faceDown ? "back" : tile.code,
        MELD_TILE_LENGTH,
      );
      if (!faceDown) {
        /* 副露里的宝牌和财神也要闪，和牌河、手牌一个节奏。 */
        if (
          isDoraTile(tile.code, view.dora_indicators ?? []) ||
          isJokerTile(tile.code, view.joker_code)
        ) {
          markTileAsDora(runtime, group);
        }
        registerTableTile(runtime, group, tile.code);
      }
      if (addedBesideCalled && calledTilePosition) {
        group.position.copy(
          addedKanTilePosition(
            calledTilePosition,
            relative,
            runtime.tileWidthRatio,
            runtime.tileScale,
          ),
        );
      } else {
        group.position.copy(
          meldTilePosition(
            relative,
            cursor,
            rotated,
            runtime.tileWidthRatio,
            runtime.tileScale,
          ),
        );
      }
      group.rotation.y =
        relative * (Math.PI / 2) +
        calledRotation;
      rootTile(runtime, group);
      if (!addedBesideCalled && rotated) {
        calledTilePosition = group.position.clone();
      }
      if (!addedBesideCalled) {
        cursor += rotated
          ? MELD_TILE_LENGTH * runtime.tileScale
          : MELD_TILE_LENGTH *
            runtime.tileWidthRatio *
            runtime.tileScale;
      }
      if (pushing) {
        /* 整组牌共用一个起跑时间，推出去才像是一只手推的。 */
        const destination = group.position.clone();
        const start = meldPushSource(destination, relative);
        group.position.copy(start);
        group.userData.baseY = start.y;
        runtime.animations.push({
          group,
          start,
          end: destination,
          startedAt: pushStartedAt,
          duration: MELD_PUSH_MS,
        });
      }
    });
    cursor += MELD_GROUP_GAP;
  }
}
