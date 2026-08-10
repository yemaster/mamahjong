import * as THREE from "three";
import { MELD_PUSH_MS } from "../animationTiming";
import {
  OPENING_DEAL_MOVE_MS,
  OPENING_DEAL_STEP_MS,
  TILE_STAND_UP_MS,
} from "./constants";
import { rotateAroundTable } from "./geometry";

/** How far a standing tile leans back towards the table centre, in radians. */
const HAND_STAND_LEAN = 0.1;

/**
 * Which local axis carries the white face of a tile lying flat at rotation 0.
 *
 * A "back" mesh is modelled upside down so a concealed tile shows green when it
 * rests on the table, which mirrors every angle derived from it.
 */
function handFaceAxis(backMesh: boolean): number {
  return backMesh ? -1 : 1;
}

/**
 * Resting angle of a standing hand: the white face points at its owner and the
 * green back at the table centre, leaning slightly inwards.
 */
export function standingHandTilt(backMesh: boolean): number {
  return handFaceAxis(backMesh) * (Math.PI / 2) - HAND_STAND_LEAN;
}

/**
 * 牌面正对镜头的立牌角度。
 *
 * 平躺的牌牌面朝上（局部 +Y），绕 X 轴转 θ 之后朝向 `(0, cosθ, sinθ)`；要牌面和
 * 屏幕平行，就得让它朝着取景方向的反向。自家的手牌不绕 Y 轴转，所以一个 X 轴
 * 角度就够。
 *
 * 只有主视角摸的那张牌用它：那张牌落地之后要原地换成二维手牌里的一格，牌面不
 * 正对镜头就换不平——三维那点俯角会把牌面压扁一截，二维的牌面却是正的。
 */
export function billboardHandTilt(camera: THREE.Camera): number {
  const forward = new THREE.Vector3(0, 0, -1).applyQuaternion(
    camera.quaternion,
  );
  return Math.atan2(-forward.z, -forward.y);
}

/**
 * 摊牌 turns the tile down and inwards until the white face is up; 盖牌 turns it
 * down and outwards until the white face meets the cloth. Both are reached by
 * continuing in one direction from {@link standingHandTilt}, never by
 * unwinding, so the tile always falls the way it visually should.
 */
export function settlementHandTilt(faceUp: boolean, backMesh: boolean): number {
  const axis = handFaceAxis(backMesh);
  return faceUp
    ? ((axis - 1) / 2) * Math.PI
    : ((axis + 1) / 2) * Math.PI;
}

/**
 * 牌山里的牌是盖着平躺的，和被扣下去的手牌是同一个角度。
 *
 * 发牌和摸牌都从这个角度出发：牌离开牌山时还盖着，落到手上才翻起来立住。
 */
export function coveredHandTilt(backMesh: boolean): number {
  return settlementHandTilt(false, backMesh);
}

/** 翻起来是一个干脆的减速，不回弹——回弹会让牌穿过桌面。 */
export function standUpEase(progress: number): number {
  const clamped = THREE.MathUtils.clamp(progress, 0, 1);
  return 1 - Math.pow(1 - clamped, 3);
}

/** Fraction of the fall spent toppling before the tile hits the felt. */
const FALL_IMPACT = 0.62;
/** How much of the travel the tile springs back on impact. */
const FALL_REBOUND = 0.11;
/** 起手先顿一下再猛砸下去，指数越大前段越慢、落地越狠。 */
const FALL_POWER = 2.7;

/** 摊牌 260ms、盖牌 230ms，整把牌一起倒。 */
export const SETTLEMENT_REVEAL_MS = 260;
export const SETTLEMENT_COVER_MS = 230;

/*
 * ── 自摸甩牌 ──
 * 自摸不是在原位翻开，而是把那张牌从高高的地方砸到桌上：整段就是一次自由落体，
 * 落地就停——牌不是弹簧，砸完不弹。砸下去那一下的力量由 `impact` 交代：牌底下
 * 扬起一层灰，同时整个镜头被撞得一颤。
 * 落点在手牌那条基线上，和其余的牌摊成一排。
 */
/** 从出手到砸在桌上的时间。落地即止，后面没有别的动作。 */
export const TSUMO_THROW_MS = 480;
/**
 * 出手的高度，单位是场景里的世界坐标。
 *
 * 相机是斜俯视的，竖直方向的位移到屏幕上会被压掉大半，所以这个值必须给得足够
 * 大（几个牌长），落下来才看得出是从高处扔的，而不是原地掉了一下。
 */
export const TSUMO_THROW_ARC = 3.2;

/**
 * 甩出去的牌离桌面有多高，`0` 到 `1` 之间，按 {@link TSUMO_THROW_ARC} 放大。
 *
 * 起手就在最高处——这是一记从上往下的砸，不是先抬起来再落下，所以进度 `0` 处
 * 是满高度。之后按自由落体加速掉，砸到桌面刚好归零，不再抬起来。
 */
export function tsumoThrowArc(progress: number): number {
  const clamped = THREE.MathUtils.clamp(progress, 0, 1);
  /* 顶上那一下几乎没动，越掉越快，砸下去才有重量。 */
  return 1 - clamped * clamped;
}

/**
 * 甩牌的水平位移和摊平跟着下落走：牌在半空还立着，贴到桌面那一下才甩平，
 * 落地的同时正好到位。
 */
export function tsumoThrowEase(progress: number): number {
  const clamped = THREE.MathUtils.clamp(progress, 0, 1);
  return clamped * clamped;
}

/**
 * Timing curve for a falling tile.
 *
 * The tile breaks away, accelerates like it is being pulled over, slams into
 * the felt at full speed and rattles twice before settling. The rebound uses
 * an absolute sine so the tile never rotates past flat and through the table.
 */
export function settlementFallEase(progress: number): number {
  const clamped = THREE.MathUtils.clamp(progress, 0, 1);
  if (clamped <= FALL_IMPACT) {
    const t = clamped / FALL_IMPACT;
    return Math.pow(t, FALL_POWER);
  }
  const t = (clamped - FALL_IMPACT) / (1 - FALL_IMPACT);
  return 1 - FALL_REBOUND * (1 - t) * Math.abs(Math.sin(t * Math.PI * 3));
}

/**
 * How far the tile travels along the seat's outward axis while it falls.
 *
 * 摊牌 hinges on the bottom outer edge so the hand ends up nearer the centre;
 * 盖牌 hinges on the bottom inner edge so it ends up nearer its owner.
 */
export function settlementHandShift(
  faceUp: boolean,
  tileLength: number,
  tileDepth: number,
): number {
  const travel = (tileLength - tileDepth) / 2;
  return faceUp ? -travel : travel;
}

/** 副露推牌：起点比副露位更靠手牌一侧，也更贴近自己。 */
const MELD_PUSH_INWARD = 0.62;
const MELD_PUSH_OUTWARD = 0.18;

export { MELD_PUSH_MS };

/**
 * 吃碰杠落位前的起点：从手牌右端推出去。
 *
 * 加杠不走这里——它只是往已经摆好的碰上再叠一张，没有推的动作。
 */
export function meldPushSource(
  destination: THREE.Vector3,
  relative: number,
): THREE.Vector3 {
  const offset = new THREE.Vector3(
    -MELD_PUSH_INWARD,
    0,
    MELD_PUSH_OUTWARD,
  );
  rotateAroundTable(offset, relative);
  return destination.clone().add(offset);
}

export function openingDealOrder(
  tileIndex: number,
  seat: number,
  dealer: number,
  seatCount: number,
): number {
  const seatOffset = (seat - dealer + seatCount) % seatCount;
  if (tileIndex < 12) {
    return (
      Math.floor(tileIndex / 4) * seatCount * 4 +
      seatOffset * 4 +
      (tileIndex % 4)
    );
  }
  if (tileIndex === 12) return seatCount * 12 + seatOffset;
  return seatCount * 13 + Math.max(0, tileIndex - 13);
}

export function openingDealStep(
  tileIndex: number,
  seat: number,
  dealer: number,
  seatCount: number,
): number {
  const seatOffset = (seat - dealer + seatCount) % seatCount;
  if (tileIndex < 12) {
    return Math.floor(tileIndex / 4) * seatCount + seatOffset;
  }
  if (tileIndex === 12) return seatCount * 3 + seatOffset;
  return seatCount * 4 + Math.max(0, tileIndex - 13);
}

/** 最后一张牌翻起来立住为止，再留一点白。 */
export function openingDealDuration(seatCount: number): number {
  const lastStep = seatCount * 4;
  return (
    lastStep * OPENING_DEAL_STEP_MS +
    OPENING_DEAL_MOVE_MS +
    TILE_STAND_UP_MS +
    180
  );
}

export function openingDealArrival(
  tileIndex: number,
  seat: number,
  dealer: number,
  seatCount: number,
): number {
  return (
    openingDealStep(tileIndex, seat, dealer, seatCount) *
      OPENING_DEAL_STEP_MS +
    OPENING_DEAL_MOVE_MS
  );
}
