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

const VERTEX_SHADER = `
varying vec2 vUv;
void main() {
  vUv = uv;
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}
`;

const FRAGMENT_SHADER = `
uniform float uProgress;
varying vec2 vUv;
void main() {
  vec2 offset = vUv - vec2(0.5);
  /* 以落点为心的半径，1.0 正好是这块面片的内切圆。 */
  float r = length(offset) * 2.0;
  if (r > 1.0) discard;
  float angle = atan(offset.y, offset.x);
  /* 尘头被啃得参差不齐，正圆一看就是特效。 */
  float wobble =
    0.80 +
    0.11 * sin(angle * 5.0 + 1.7) +
    0.06 * sin(angle * 9.0 - 0.6) +
    0.04 * sin(angle * 14.0 + 2.9);
  /* 一开始冲得急，很快就没劲了。 */
  float spread = 1.0 - pow(1.0 - uProgress, 3.0);
  float front = spread * wobble;
  /* 尘头之内才有灰，外面干净。 */
  float body = 1.0 - smoothstep(front - 0.13, front, r);
  /* 中心先散掉，浓的部分跟着尘头往外走。 */
  float hollow = smoothstep(front * 0.25, front * 0.92, r);
  /* 一圈里有浓有淡，不是均匀一片。 */
  float patch =
    0.52 + 0.48 * sin(angle * 7.0 + 0.9) * sin(angle * 3.0 - 2.1);
  float dust = body * mix(0.28, 1.0, hollow) * mix(0.45, 1.0, patch);
  /* 落点底下被砸实的那一小片暗影，比灰散得还快。 */
  float contact =
    (1.0 - smoothstep(0.0, 0.3, r)) * pow(1.0 - min(uProgress * 2.4, 1.0), 2.0);
  float dustAlpha = dust * 0.3 * pow(1.0 - uProgress, 1.4);
  float shadeAlpha = contact * 0.36;
  float alpha = dustAlpha + shadeAlpha;
  if (alpha <= 0.001) discard;
  /* 两层各自的颜色按各自的浓度混，暗影压在灰底下。 */
  vec3 color =
    (vec3(0.84, 0.82, 0.75) * dustAlpha + vec3(0.02, 0.05, 0.03) * shadeAlpha) /
    alpha;
  gl_FragColor = vec4(color, alpha);
  #include <colorspace_fragment>
}
`;

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
  const material = new THREE.ShaderMaterial({
    uniforms: { uProgress: { value: 0 } },
    vertexShader: VERTEX_SHADER,
    fragmentShader: FRAGMENT_SHADER,
    transparent: true,
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
  runtime.root.add(mesh);
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
    const uniform = impact.material.uniforms.uProgress;
    if (uniform) uniform.value = progress;
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
