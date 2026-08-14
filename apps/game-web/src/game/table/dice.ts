import * as THREE from "three";
import { RoundedBoxGeometry } from "three/addons/geometries/RoundedBoxGeometry.js";
import type { TableRuntime } from "./types";

/**
 * 掷骰是开局前的一个交代，不是一段表演：滚太多圈等于让每一局都从等待开始。
 * 两颗只错开一点点，第二颗停稳的时刻就是 `DICE_SETTLE_MS`。
 */
const DICE_ROLL_MS = 700;
const DICE_STAGGER_MS = 70;

/** 从投骰那一刻算起，两颗骰子都停稳需要多久。 */
export const DICE_SETTLE_MS = DICE_ROLL_MS + DICE_STAGGER_MS + 40;

/** 停稳之后留的半拍，让人看清点数再翻宝牌。 */
export const DORA_FLIP_DELAY_MS = 180;

/** 宝牌指示牌绕自身长轴转半圈的时长。 */
export const DORA_FLIP_MS = 320;

/** 骰子加翻宝牌整段：走完就该开始取牌了。 */
export const OPENING_DICE_MS =
  DICE_SETTLE_MS + DORA_FLIP_DELAY_MS + DORA_FLIP_MS;

/** 开局掷骰：两颗骰子从桌角滚到桌心，停在给定的点数上。 */
export function addTableDice(
  runtime: TableRuntime,
  values: [number, number],
): void {
  const faceOrder = [3, 4, 2, 5, 1, 6];
  const now = performance.now();
  values.forEach((value, index) => {
    const materials = faceOrder.map(
      (faceValue) =>
        new THREE.MeshStandardMaterial({
          map: dieFaceTexture(faceValue),
          roughness: 0.76,
          metalness: 0,
        }),
    );
    const die = new THREE.Mesh(
      new RoundedBoxGeometry(0.44, 0.44, 0.44, 5, 0.064),
      materials,
    );
    die.castShadow = false;
    die.receiveShadow = false;

    const yaw = new THREE.Quaternion().setFromAxisAngle(
      new THREE.Vector3(0, 1, 0),
      index === 0 ? 0.16 : -0.18,
    );
    const finalRotation = dieQuaternion(value).multiply(yaw);
    const finalEuler = new THREE.Euler().setFromQuaternion(
      finalRotation,
      "XYZ",
    );
    const startPosition = new THREE.Vector3(
      index === 0 ? -1.15 : -0.5,
      1.18 + index * 0.16,
      3.2 + index * 0.18,
    );
    const endPosition = new THREE.Vector3(
      index === 0 ? -0.27 : 0.27,
      0.3,
      1.5 + index * 0.07,
    );
    const startEuler = new THREE.Vector3(
      0.35 + index * 0.42,
      -0.5 - index * 0.31,
      0.2 + index * 0.28,
    );
    /* 三轴都翻，但一两圈就落定。 */
    const endEuler = new THREE.Vector3(
      finalEuler.x + Math.PI * (3 + index),
      finalEuler.y + Math.PI * (4 + index),
      finalEuler.z + Math.PI * (2 + index),
    );
    die.position.copy(startPosition);
    die.rotation.set(startEuler.x, startEuler.y, startEuler.z);
    runtime.renderTarget.add(die);
    runtime.diceRolls.push({
      object: die,
      startPosition,
      endPosition,
      startEuler,
      endEuler,
      finalRotation,
      startedAt: now + index * DICE_STAGGER_MS,
      duration: DICE_ROLL_MS + index * 40,
    });
  });
}

function dieFaceTexture(value: number): THREE.CanvasTexture {
  const canvas = document.createElement("canvas");
  canvas.width = 128;
  canvas.height = 128;
  const context = canvas.getContext("2d")!;
  context.fillStyle = "#ece7dc";
  context.fillRect(0, 0, 128, 128);
  const positions: Record<number, [number, number][]> = {
    1: [[64, 64]],
    2: [[35, 35], [93, 93]],
    3: [[35, 35], [64, 64], [93, 93]],
    4: [[35, 35], [93, 35], [35, 93], [93, 93]],
    5: [[35, 35], [93, 35], [64, 64], [35, 93], [93, 93]],
    6: [[35, 29], [35, 64], [35, 99], [93, 29], [93, 64], [93, 99]],
  };
  for (const [x, y] of positions[value] ?? []) {
    context.beginPath();
    context.fillStyle = value === 1 ? "#a13a43" : "#282622";
    context.arc(x, y, 9.5, 0, Math.PI * 2);
    context.fill();
  }
  const texture = new THREE.CanvasTexture(canvas);
  texture.colorSpace = THREE.SRGBColorSpace;
  return texture;
}

function dieQuaternion(value: number): THREE.Quaternion {
  const [x, z] =
    ({
      1: [0, 0],
      2: [0, -Math.PI / 2],
      3: [0, Math.PI / 2],
      4: [Math.PI / 2, 0],
      5: [-Math.PI / 2, 0],
      6: [Math.PI, 0],
    }[value] ?? [0, 0]);
  return new THREE.Quaternion().setFromEuler(new THREE.Euler(x, 0, z));
}
