import * as THREE from "three";
import { RoundedBoxGeometry } from "three/addons/geometries/RoundedBoxGeometry.js";
import { normalizeTileCode } from "../tileAssets";
import { TILE_DEPTH_RATIO } from "./constants";
import { makeDoraShine } from "./doraShine";
import type { TableRuntime, TileGeometrySet } from "./types";

/**
 * 一张麻将牌：上下两层加一圈接缝，牌面贴图单独铺一层平面。
 *
 * 返回的 group 结构是 group → pivot → body，pivot 负责结算时绕底边倒下，
 * group 负责在桌面上的摆位，两层分开才不会互相干扰。
 */
export function makeTile(
  runtime: TableRuntime,
  code: string,
  length: number,
): THREE.Group {
  const width = length * runtime.tileWidthRatio;
  const depth = length * TILE_DEPTH_RATIO;
  const edgeRadius = Math.min(
    0.04,
    Math.min(width, length) * 0.075,
    depth * 0.14,
  );
  const normalized = code === "back" ? "back" : normalizeTileCode(code);
  const isBack = normalized === "back";
  const faceTexture = runtime.textures.get(normalized);
  const upperSide = sharedTileMaterial(
    runtime,
    "body:upper",
    () =>
      new THREE.MeshStandardMaterial({
        color: 0xc9c9c6,
        roughness: 0.55,
      }),
  );
  const back = sharedTileMaterial(
    runtime,
    "body:back",
    () =>
      new THREE.MeshStandardMaterial({
        color: 0x689974,
        roughness: 0.64,
      }),
  );
  /* 牌背用不到白色牌面；旧实现仍为每张背牌 new 一份且永远无法释放。 */
  const face = isBack
    ? null
    : new THREE.MeshBasicMaterial({ color: 0xdfdfdc });
  const artwork = isBack
    ? null
    : sharedTileMaterial(
        runtime,
        `artwork:${normalized}`,
        () =>
          new THREE.MeshBasicMaterial({
            color: 0xffffff,
            map: faceTexture,
            /* 牌面是镂空图案，用 alpha cutout 进入不透明队列；这样几百张牌不再
               每帧参与透明物体距离排序，新增一张牌也不会打乱整桌绘制顺序。 */
            transparent: false,
            alphaTest: 0.035,
            alphaToCoverage: true,
            depthWrite: true,
            /* 把图案稳定地压到牌坯前面，杜绝动画时与白底争抢深度。 */
            polygonOffset: true,
            polygonOffsetFactor: -2,
            polygonOffsetUnits: -2,
            toneMapped: false,
          }),
      );
  const faceLayerDepth = depth * 0.58;
  const backLayerDepth = depth - faceLayerDepth;
  const upperLayerDepth = isBack ? backLayerDepth : faceLayerDepth;
  const lowerLayerDepth = isBack ? faceLayerDepth : backLayerDepth;
  const upperRadius = Math.min(edgeRadius, upperLayerDepth * 0.48);
  const lowerRadius = Math.min(edgeRadius, lowerLayerDepth * 0.48);
  const geometryKey = [
    isBack ? "back" : "face",
    length,
    runtime.tileWidthRatio,
  ].join(":");
  let geometries = runtime.tileGeometries.get(geometryKey);
  if (!geometries) {
    const upperGeometry = new RoundedBoxGeometry(
      width * 1.006,
      upperLayerDepth,
      length * 1.006,
      5,
      upperRadius,
    );
    const lowerGeometry = new RoundedBoxGeometry(
      width,
      lowerLayerDepth,
      length,
      5,
      lowerRadius,
    );
    const seamGeometry = roundedSeamGeometry(
      width * 1.006,
      length * 1.006,
      depth * 0.12,
      edgeRadius,
    );
    const artworkGeometry = isBack
      ? null
      : artworkPlaneGeometry(width, length);
    geometries = {
      upper: upperGeometry,
      lower: lowerGeometry,
      seam: seamGeometry,
      artwork: artworkGeometry,
    };
    markSharedTileGeometries(geometries);
    runtime.tileGeometries.set(geometryKey, geometries);
  }
  const upperMaterial = isBack || !face
    ? back
    : [upperSide, upperSide, face, upperSide, upperSide, upperSide];
  const upper = new THREE.Mesh(geometries.upper, upperMaterial);
  upper.position.y = (depth - upperLayerDepth) / 2;
  upper.castShadow = false;
  upper.receiveShadow = false;

  const lower = new THREE.Mesh(
    geometries.lower,
    isBack ? upperSide : back,
  );
  lower.position.y = -(depth - lowerLayerDepth) / 2;
  lower.castShadow = false;
  lower.receiveShadow = false;

  const seamDepth = depth * 0.12;
  const layerBoundary = (lowerLayerDepth - upperLayerDepth) / 2;
  const upperSeam = new THREE.Mesh(
    geometries.seam,
    isBack ? back : upperSide,
  );
  upperSeam.position.y = layerBoundary + seamDepth / 2;
  const lowerSeam = new THREE.Mesh(
    geometries.seam,
    isBack ? upperSide : back,
  );
  lowerSeam.position.y = layerBoundary - seamDepth / 2;
  upperSeam.castShadow = false;
  upperSeam.receiveShadow = false;
  lowerSeam.castShadow = false;
  lowerSeam.receiveShadow = false;

  const body = new THREE.Group();
  let groupArtworkMesh: THREE.Mesh | null = null;
  body.add(upper, lower, upperSeam, lowerSeam);
  if (artwork && geometries.artwork) {
    const artworkMesh = new THREE.Mesh(
      geometries.artwork,
      artwork,
    );
    artworkMesh.rotation.x = -Math.PI / 2;
    /* 0.001 在部分移动端的 16/24 位深度缓冲里不足以分层。 */
    artworkMesh.position.y = depth / 2 + Math.max(0.004, depth * 0.018);
    artworkMesh.renderOrder = 1;
    artworkMesh.castShadow = false;
    artworkMesh.receiveShadow = false;
    body.add(artworkMesh);
    groupArtworkMesh = artworkMesh;
  }
  const pivot = new THREE.Group();
  body.position.y = depth / 2;
  pivot.add(body);
  const group = new THREE.Group();
  group.add(pivot);
  group.scale.setScalar(runtime.tileScale);
  group.userData.tileBody = body;
  group.userData.tilePivot = pivot;
  group.userData.tileDepth = depth;
  group.userData.tileWidth = width;
  group.userData.tileLength = length;
  group.userData.faceMesh = upper;
  group.userData.faceMaterial = isBack ? back : artwork;
  /* 牌面那块白底，点亮同种牌时染的是它，不是上面的图案。 */
  group.userData.facePlateMaterial = isBack ? null : face;
  group.userData.artworkMesh = groupArtworkMesh;
  group.userData.hovered = false;
  group.userData.hoverLift = 0;
  return group;
}

function sharedTileMaterial<T extends THREE.Material>(
  runtime: TableRuntime,
  key: string,
  create: () => T,
): T {
  const cached = runtime.tileMaterials.get(key) as T | undefined;
  if (cached) return cached;
  const material = create();
  material.userData.shared = true;
  runtime.tileMaterials.set(key, material);
  return material;
}

function artworkPlaneGeometry(
  width: number,
  length: number,
): THREE.PlaneGeometry {
  const svgAspect = 19 / 26;
  /* 墙牌和小牌需要放大图案比例，让宝牌指示牌更清晰可见。 */
  const artworkScale = length <= 0.56 ? 0.98 : 0.94;
  const artworkWidth = width * artworkScale;
  return new THREE.PlaneGeometry(artworkWidth, artworkWidth / svgAspect);
}

function markSharedTileGeometries(geometries: TileGeometrySet): void {
  geometries.upper.userData.sharedTileGeometry = true;
  geometries.lower.userData.sharedTileGeometry = true;
  geometries.seam.userData.sharedTileGeometry = true;
  if (geometries.artwork) {
    geometries.artwork.userData.sharedTileGeometry = true;
  }
}

/**
 * 把一整张牌压暗一档。
 *
 * 牌谱里用来分手切和摸切：摸切的牌在牌河里整体沉下去一层，扫一眼就知道这一巡是
 * 顺手推掉的还是从手里挑出来的。牌身、牌面、图案一起压，只暗牌面会看着像脏了。
 *
 * 两条得记住的：
 * - 普通牌身/图案现在由 runtime 共用；变暗前必须先为这一张做写时复制；
 * - 一张牌的几个面共用同一个材质实例（`upperSide` 就挂在五个面上），所以得去重，
 *   否则同一块颜色会被连乘好几次，直接压成黑的；
 * - 要在 `registerTableTile` 之前调——点亮同种牌那套记的是调用当时的颜色，晚了
 *   压暗就会被高亮还原掉。
 */
export function dimTile(group: THREE.Group, factor: number): void {
  const replacements = new Map<THREE.Material, THREE.Material>();
  const seen = new Set<THREE.Material>();
  group.traverse((object) => {
    const mesh = object as THREE.Mesh;
    if (!mesh.isMesh) return;
    const originalMaterials = Array.isArray(mesh.material)
      ? mesh.material
      : [mesh.material];
    const materials = originalMaterials.map((original) => {
      /* 扫光 Shader 也标记为 shared，但它没有颜色且必须继续共用同一个 uPhase。 */
      if (
        !original.userData.shared ||
        !("color" in original)
      ) {
        return original;
      }
      const existing = replacements.get(original);
      if (existing) return existing;
      const clone = original.clone();
      clone.userData = { ...original.userData, shared: false };
      replacements.set(original, clone);
      return clone;
    });
    mesh.material = Array.isArray(mesh.material) ? materials : materials[0]!;
    for (const material of materials) {
      if (!material || seen.has(material)) continue;
      seen.add(material);
      (material as THREE.MeshStandardMaterial).color?.multiplyScalar(factor);
    }
  });
  const faceMaterial = group.userData.faceMaterial as THREE.Material | null;
  const facePlateMaterial = group.userData.facePlateMaterial as
    | THREE.Material
    | null;
  if (faceMaterial && replacements.has(faceMaterial)) {
    group.userData.faceMaterial = replacements.get(faceMaterial);
  }
  if (facePlateMaterial && replacements.has(facePlateMaterial)) {
    group.userData.facePlateMaterial = replacements.get(facePlateMaterial);
  }
}

function roundedSeamGeometry(
  width: number,
  length: number,
  depth: number,
  radius: number,
): THREE.ExtrudeGeometry {
  const halfWidth = width / 2;
  const halfLength = length / 2;
  const corner = Math.min(radius, halfWidth, halfLength);
  const shape = new THREE.Shape();
  shape.moveTo(-halfWidth + corner, -halfLength);
  shape.lineTo(halfWidth - corner, -halfLength);
  shape.quadraticCurveTo(
    halfWidth,
    -halfLength,
    halfWidth,
    -halfLength + corner,
  );
  shape.lineTo(halfWidth, halfLength - corner);
  shape.quadraticCurveTo(
    halfWidth,
    halfLength,
    halfWidth - corner,
    halfLength,
  );
  shape.lineTo(-halfWidth + corner, halfLength);
  shape.quadraticCurveTo(
    -halfWidth,
    halfLength,
    -halfWidth,
    halfLength - corner,
  );
  shape.lineTo(-halfWidth, -halfLength + corner);
  shape.quadraticCurveTo(
    -halfWidth,
    -halfLength,
    -halfWidth + corner,
    -halfLength,
  );
  shape.closePath();

  const geometry = new THREE.ExtrudeGeometry(shape, {
    depth,
    bevelEnabled: false,
    curveSegments: 5,
    steps: 1,
  });
  geometry.translate(0, 0, -depth / 2);
  geometry.rotateX(-Math.PI / 2);
  return geometry;
}

/**
 * 给一张宝牌盖上那道扫光，并登记到 runtime 里由动画循环统一推进。
 *
 * 牌河、副露和手牌走的是同一份光带，桌上桌下闪成一个节奏。
 */
export function markTileAsDora(
  runtime: TableRuntime,
  group: THREE.Group,
): void {
  let face = group.userData.faceMaterial as
    | THREE.MeshBasicMaterial
    | undefined;
  if (!face) return;
  if (face.userData.shared) {
    const clone = face.clone();
    clone.userData = { ...face.userData, shared: false };
    const artworkMesh = group.userData.artworkMesh as THREE.Mesh | undefined;
    if (artworkMesh) artworkMesh.material = clone;
    group.userData.faceMaterial = clone;
    face = clone;
  }
  face.color.setRGB(0.92, 0.95, 1);
  const body = group.userData.tileBody as THREE.Group | undefined;
  const width = group.userData.tileWidth as number | undefined;
  const length = group.userData.tileLength as number | undefined;
  const depth = group.userData.tileDepth as number | undefined;
  if (!body || width == null || length == null || depth == null) return;
  const shine = makeDoraShine(runtime.doraShine, width, length);
  /* 压在图案那一层之上，牌面上所有东西都会被它扫到。 */
  shine.position.y = depth / 2 + Math.max(0.007, depth * 0.028);
  body.add(shine);
}

export function tileBody(group: THREE.Group): THREE.Group {
  return group.userData.tileBody as THREE.Group;
}

export function tileFaceMesh(group: THREE.Group): THREE.Mesh {
  return group.userData.faceMesh as THREE.Mesh;
}

export function rootTile(runtime: TableRuntime, group: THREE.Group): void {
  runtime.renderTarget.add(group);
}
