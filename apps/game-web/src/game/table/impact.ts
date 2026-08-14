import * as THREE from "three";
import type { CameraShake, TableImpact, TableRuntime } from "./types";

/**
 * 牌砸在桌面上那一下。
 *
 * 分两件事：桌布上扬起的一层灰，和整个画面被震的那一下。光靠桌面上的效果撑不
 * 起冲击感——一圈干净的亮环看着像法术，不像牌砸下来；真正让人觉得「砸到了」的
 * 是镜头猛地一颤。所以这里两样一起给，同一时刻起。
 *
 * 灰也不是一圈环：从落点往外推的是一层参差不齐、有浓有淡的暖灰尘，边缘被角度
 * 噪声啃出缺口；落点底下另压一小片暗影，是桌布被砸实的那一下，很快就化开。
 */
/** 灰扬起到散尽。要短——尘土是「噗」一下，挂久了就假。 */
export const IMPACT_DUST_MS = 260;
/** 灰能推多远，按砸下来那张牌的长度算。 */
export const IMPACT_DUST_SPREAD = 1.7;
/** 镜头颤的时长，比灰还短一点，砸完立刻稳住。 */
export const IMPACT_SHAKE_MS = 240;
/** 镜头颤的幅度，单位是场景里的世界坐标。 */
export const IMPACT_SHAKE_AMPLITUDE = 0.17;

/**
 * 扬尘只需要一张很小的透明纹理。用 Canvas 预生成后交给 Three 的标准材质，既
 * 避免不同 GPU 驱动编译自定义片元 shader 的差异，也能让多次砸牌共用一份贴图。
 */
export function createImpactDustTexture(): THREE.CanvasTexture {
  const canvas = document.createElement("canvas");
  canvas.width = 128;
  canvas.height = 128;
  const context = canvas.getContext("2d");
  if (!context) throw new Error("无法创建砸牌扬尘纹理");
  context.translate(64, 64);

  const contact = context.createRadialGradient(0, 0, 0, 0, 0, 27);
  contact.addColorStop(0, "rgba(4, 13, 8, 0.58)");
  contact.addColorStop(0.42, "rgba(32, 40, 31, 0.28)");
  contact.addColorStop(1, "rgba(32, 40, 31, 0)");
  context.fillStyle = contact;
  context.fillRect(-64, -64, 128, 128);

  /* 确定性的大小与距离起伏，边缘保持参差，不使用随机数造成每次观感不同。 */
  for (let index = 0; index < 48; index += 1) {
    const angle = (index / 48) * Math.PI * 2;
    const distance = 35 + Math.sin(index * 2.17) * 7;
    const radius = 7 + (Math.sin(index * 1.31 + 0.8) + 1) * 2.5;
    const alpha = 0.12 + (Math.sin(index * 0.83) + 1) * 0.045;
    context.beginPath();
    context.fillStyle = `rgba(214, 209, 191, ${alpha})`;
    context.arc(
      Math.cos(angle) * distance,
      Math.sin(angle) * distance,
      radius,
      0,
      Math.PI * 2,
    );
    context.fill();
  }

  const texture = new THREE.CanvasTexture(canvas);
  texture.name = "table-impact-dust-texture";
  texture.colorSpace = THREE.SRGBColorSpace;
  texture.userData.sharedImpactTexture = true;
  return texture;
}

/**
 * 镜头被砸得偏出去多少，横竖各一份，`-1` 到 `1` 之间，按幅度放大。
 *
 * 砸下去那一瞬就偏到最大，之后来回两三下迅速收住——这是被撞了一下，不是在晃。
 * 横竖用不同的频率，才不像沿着一条直线来回拉。
 */
export function cameraShakeOffset(progress: number): {
  x: number;
  y: number;
} {
  const clamped = THREE.MathUtils.clamp(progress, 0, 1);
  const decay = Math.pow(1 - clamped, 2);
  return {
    x: Math.sin(clamped * Math.PI * 6.4) * decay * 0.55,
    y: Math.cos(clamped * Math.PI * 4.6) * decay,
  };
}

/**
 * 在桌面某处排一次砸击：扬灰加震镜头。
 *
 * `startedAt` 可以排在将来：牌还在往下掉的时候就把这一下登记好，等它砸到桌面
 * 的那一刻自己开始，不必再回来补一次。
 */
export function spawnTableImpact(
  runtime: TableRuntime,
  position: THREE.Vector3,
  radius: number,
  startedAt: number,
): void {
  const material = new THREE.MeshBasicMaterial({
    name: "table-impact-dust",
    map: runtime.impactDustTexture,
    transparent: true,
    opacity: 0,
    depthWrite: false,
    toneMapped: false,
  });
  const mesh = new THREE.Mesh(
    new THREE.PlaneGeometry(radius * 2, radius * 2),
    material,
  );
  mesh.position.copy(position);
  mesh.rotation.x = -Math.PI / 2;
  /* 压在桌布之上、牌之下，灰是从牌底下扬出来的。 */
  mesh.renderOrder = 1;
  mesh.visible = false;
  mesh.scale.setScalar(0.18);
  runtime.renderTarget.add(mesh);
  runtime.impacts.push({
    mesh,
    material,
    startedAt,
    duration: IMPACT_DUST_MS,
  });
  runtime.shake = {
    startedAt,
    duration: IMPACT_SHAKE_MS,
    amplitude: IMPACT_SHAKE_AMPLITUDE,
  };
}

/**
 * 推进桌面上的灰，散完的就地拆掉。
 *
 * @returns 还没散完的那些，直接赋回 `runtime.impacts`。
 */
export function advanceTableImpacts(
  impacts: TableImpact[],
  now: number,
): TableImpact[] {
  if (impacts.length === 0) return impacts;
  return impacts.filter((impact) => {
    const progress = (now - impact.startedAt) / impact.duration;
    /* 牌还没落地，这一层灰先藏着。 */
    if (progress < 0) return true;
    if (progress >= 1) {
      impact.mesh.removeFromParent();
      impact.mesh.geometry.dispose();
      impact.material.dispose();
      return false;
    }
    impact.mesh.visible = true;
    const spread = 1 - Math.pow(1 - progress, 3);
    impact.mesh.scale.setScalar(0.18 + spread * 0.82);
    impact.material.opacity = 0.62 * Math.pow(1 - progress, 1.4);
    return true;
  });
}

/**
 * 把镜头颤的那一下抹到相机上。
 *
 * 只挪相机的位置、不动朝向，画面就是整块被撞偏，而不是绕着桌子转。颤完把相机
 * 放回基准位，免得留下一点偏移一直挂着。
 *
 * @returns 还没颤完的那一下；`null` 表示已经稳住了。
 */
export function advanceCameraShake(
  runtime: TableRuntime,
  now: number,
): CameraShake | null {
  const shake = runtime.shake;
  if (!shake) return null;
  const progress = (now - shake.startedAt) / shake.duration;
  if (progress < 0) return shake;
  if (progress >= 1) {
    runtime.camera.position.copy(runtime.cameraBase);
    return null;
  }
  const offset = cameraShakeOffset(progress);
  runtime.camera.position.set(
    runtime.cameraBase.x + offset.x * shake.amplitude,
    runtime.cameraBase.y + offset.y * shake.amplitude,
    runtime.cameraBase.z,
  );
  return shake;
}
