import * as THREE from "three";
import { RoundedBoxGeometry } from "three/addons/geometries/RoundedBoxGeometry.js";
import { normalizeTileCode } from "../tileAssets";
import { TILE_DEPTH_RATIO } from "./constants";
import { makeDoraShine } from "./doraShine";
import type { TableRuntime } from "./types";

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
  const upperSide = new THREE.MeshStandardMaterial({
    color: 0xc9c9c6,
    roughness: 0.55,
  });
  const back = new THREE.MeshStandardMaterial({
    color: 0x689974,
    roughness: 0.64,
  });
  const face = new THREE.MeshBasicMaterial({
    color: 0xdfdfdc,
  });
  const artwork = isBack
    ? null
    : new THREE.MeshBasicMaterial({
        color: 0xffffff,
        map: faceTexture,
        transparent: true,
        alphaTest: 0.01,
        depthWrite: false,
        toneMapped: false,
      });
  const faceLayerDepth = depth * 0.58;
  const backLayerDepth = depth - faceLayerDepth;
  const upperLayerDepth = isBack ? backLayerDepth : faceLayerDepth;
  const lowerLayerDepth = isBack ? faceLayerDepth : backLayerDepth;
  const upperRadius = Math.min(edgeRadius, upperLayerDepth * 0.48);
  const lowerRadius = Math.min(edgeRadius, lowerLayerDepth * 0.48);
  const upperGeometry = new RoundedBoxGeometry(
    width * 1.006,
    upperLayerDepth,
    length * 1.006,
    5,
    upperRadius,
  );
  const upperMaterial = isBack
    ? back
    : [upperSide, upperSide, face, upperSide, upperSide, upperSide];
  const upper = new THREE.Mesh(upperGeometry, upperMaterial);
  upper.position.y = (depth - upperLayerDepth) / 2;
  upper.castShadow = false;
  upper.receiveShadow = false;

  const lower = new THREE.Mesh(
    new RoundedBoxGeometry(width, lowerLayerDepth, length, 5, lowerRadius),
    isBack ? upperSide : back,
  );
  lower.position.y = -(depth - lowerLayerDepth) / 2;
  lower.castShadow = false;
  lower.receiveShadow = false;

  const seamDepth = depth * 0.12;
  const seamWidth = width * 1.006;
  const seamLength = length * 1.006;
  const seamGeometry = roundedSeamGeometry(
    seamWidth,
    seamLength,
    seamDepth,
    edgeRadius,
  );
  const layerBoundary = (lowerLayerDepth - upperLayerDepth) / 2;
  const upperSeam = new THREE.Mesh(
    seamGeometry,
    isBack ? back : upperSide,
  );
  upperSeam.position.y = layerBoundary + seamDepth / 2;
  const lowerSeam = new THREE.Mesh(
    seamGeometry.clone(),
    isBack ? upperSide : back,
  );
  lowerSeam.position.y = layerBoundary - seamDepth / 2;
  upperSeam.castShadow = false;
  upperSeam.receiveShadow = false;
  lowerSeam.castShadow = false;
  lowerSeam.receiveShadow = false;

  const body = new THREE.Group();
  body.add(upper, lower, upperSeam, lowerSeam);
  if (artwork) {
    const svgAspect = 19 / 26;
    /* 墙牌和小牌需要放大图案比例，让宝牌指示牌更清晰可见 */
    const artworkScale = length <= 0.56 ? 0.98 : 0.94;
    const artworkWidth = width * artworkScale;
    const artworkLength = artworkWidth / svgAspect;
    const artworkMesh = new THREE.Mesh(
      new THREE.PlaneGeometry(artworkWidth, artworkLength),
      artwork,
    );
    artworkMesh.rotation.x = -Math.PI / 2;
    artworkMesh.position.y = depth / 2 + 0.001;
    artworkMesh.renderOrder = 1;
    artworkMesh.castShadow = false;
    artworkMesh.receiveShadow = false;
    body.add(artworkMesh);
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
  group.userData.hovered = false;
  group.userData.hoverLift = 0;
  return group;
}

/**
 * 把一整张牌压暗一档。
 *
 * 牌谱里用来分手切和摸切：摸切的牌在牌河里整体沉下去一层，扫一眼就知道这一巡是
 * 顺手推掉的还是从手里挑出来的。牌身、牌面、图案一起压，只暗牌面会看着像脏了。
 *
 * 两条得记住的：
 * - 材质是每张牌自己 `new` 的，改这一张的颜色不会牵连别的牌；
 * - 一张牌的几个面共用同一个材质实例（`upperSide` 就挂在五个面上），所以得去重，
 *   否则同一块颜色会被连乘好几次，直接压成黑的；
 * - 要在 `registerTableTile` 之前调——点亮同种牌那套记的是调用当时的颜色，晚了
 *   压暗就会被高亮还原掉。
 */
export function dimTile(group: THREE.Group, factor: number): void {
  const seen = new Set<THREE.Material>();
  group.traverse((object) => {
    const mesh = object as THREE.Mesh;
    if (!mesh.isMesh) return;
    const materials = Array.isArray(mesh.material)
      ? mesh.material
      : [mesh.material];
    for (const material of materials) {
      if (!material || seen.has(material)) continue;
      seen.add(material);
      (material as THREE.MeshStandardMaterial).color?.multiplyScalar(factor);
    }
  });
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
  const face = group.userData.faceMaterial as
    | THREE.MeshBasicMaterial
    | undefined;
  if (!face) return;
  face.color.setRGB(0.92, 0.95, 1);
  const body = group.userData.tileBody as THREE.Group | undefined;
  const width = group.userData.tileWidth as number | undefined;
  const length = group.userData.tileLength as number | undefined;
  const depth = group.userData.tileDepth as number | undefined;
  if (!body || width == null || length == null || depth == null) return;
  const shine = makeDoraShine(runtime.doraShine, width, length);
  /* 压在图案那一层之上，牌面上所有东西都会被它扫到。 */
  shine.position.y = depth / 2 + 0.002;
  body.add(shine);
}

export function tileBody(group: THREE.Group): THREE.Group {
  return group.userData.tileBody as THREE.Group;
}

export function tileFaceMesh(group: THREE.Group): THREE.Mesh {
  return group.userData.faceMesh as THREE.Mesh;
}

export function rootTile(runtime: TableRuntime, group: THREE.Group): void {
  runtime.root.add(group);
}
