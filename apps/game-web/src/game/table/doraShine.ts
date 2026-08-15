import * as THREE from "three";

/**
 * 桌上宝牌的闪光。
 *
 * 二维手牌那道光是一条斜白带扫过牌面（CSS 的 `matchHandDoraShine`），桌上这层
 * 照着同一条曲线做：同样 1.4 秒一轮，前半程扫过去，后半程留白，好让牌河、副露
 * 和手牌闪成一个节奏。桌上的牌小、又是斜着看的，光带比二维那道更宽、边缘化得
 * 更开，不然细细一道扫过去只剩一条硬边。光带直接在着色器里算，不用为每张牌
 * 准备贴图。
 */
export const DORA_SHINE_PERIOD_MS = 1400;

const VERTEX_SHADER = `
varying vec2 vUv;
void main() {
  vUv = uv;
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}
`;

const FRAGMENT_SHADER = `
uniform float uPhase;
varying vec2 vUv;
void main() {
  /* 光带只在前 48% 扫过去，后面那半轮牌面是干净的。 */
  float sweep = uPhase / 0.48;
  /* 光带比牌面还宽，扫过去是一整片亮，不是一道细缝。 */
  float center = mix(-0.85, 1.85, sweep);
  /* 稍微斜一点，对上二维那道 skewX(-8deg)。 */
  float x = vUv.x + (vUv.y - 0.5) * 0.14;
  /* 高斯衰减，两头化得开，不会在牌面上留下硬边。 */
  float d = (x - center) / 0.62;
  float band = exp(-d * d * 2.4);
  /* 进出都用平滑过渡，光带别是突然出现、突然没了。 */
  float alive = smoothstep(0.0, 0.14, uPhase) * (1.0 - smoothstep(0.34, 0.48, uPhase));
  gl_FragColor = vec4(0.957, 0.984, 1.0, band * 0.4 * alive);
  #include <colorspace_fragment>
}
`;

/**
 * 整张桌子共用的那份扫光材质。
 *
 * 桌上所有宝牌本来就闪成一个节奏，一份 `uPhase` 就够。分给每张牌一份材质的话，
 * 局部层更新时也没有必要把这批材质连同 GPU 上的着色器程序删掉再编一遍。材质挂
 * 在牌桌运行时上，跟着运行时活到最后，`disposeGroup` 靠 `userData.shared` 认出它
 * 并放过。
 */
export function createDoraShineMaterial(): THREE.ShaderMaterial {
  const material = new THREE.ShaderMaterial({
    name: "dora-shine",
    uniforms: { uPhase: { value: 0 } },
    vertexShader: VERTEX_SHADER,
    fragmentShader: FRAGMENT_SHADER,
    transparent: true,
    depthWrite: false,
    polygonOffset: true,
    polygonOffsetFactor: -3,
    polygonOffsetUnits: -3,
    toneMapped: false,
  });
  material.userData.shared = true;
  return material;
}

/** 铺在牌面正上方的一层光，材质由牌桌统一提供。 */
export function makeDoraShine(
  material: THREE.ShaderMaterial,
  width: number,
  length: number,
): THREE.Mesh {
  const mesh = new THREE.Mesh(
    new THREE.PlaneGeometry(width, length),
    material,
  );
  mesh.rotation.x = -Math.PI / 2;
  mesh.renderOrder = 2;
  mesh.castShadow = false;
  mesh.receiveShadow = false;
  return mesh;
}

/** 所有宝牌共用一条时间轴，桌上闪成一片而不是各闪各的。 */
export function advanceDoraShine(
  material: THREE.ShaderMaterial,
  now: number,
): void {
  const uniform = material.uniforms.uPhase;
  if (uniform) uniform.value = (now % DORA_SHINE_PERIOD_MS) / DORA_SHINE_PERIOD_MS;
}
