import * as THREE from "three";
import {
  clientPointInViewport,
  clientRectInViewport,
} from "../../components/viewportCoordinates";
import { tileAssetPath, tileCodes } from "../tileAssets";
import { settlementFallEase, tsumoThrowArc } from "./animation";
import {
  toggleCenterConsolePoints,
  updateCenterConsoleTexture,
} from "./centerConsole";
import {
  DEFAULT_TABLECLOTH_ASSET,
  DEFAULT_TILE_SCALE,
  TILE_WIDTH_RATIO,
} from "./constants";
import {
  orthographicCameraBounds,
  tableCameraLayout,
} from "./geometry";
import { advanceDoraShine, createDoraShineMaterial } from "./doraShine";
import {
  advanceCameraShake,
  advanceTableImpacts,
  createImpactDustTexture,
} from "./impact";
import { advanceTransientTiles } from "./transient";
import type { TableCameraConfig, TableRuntime } from "./types";

/**
 * 建起 three.js 的渲染器、相机、灯光和那一条 rAF 动画循环。
 *
 * 牌桌上每一种运动（发牌、打牌、推副露、摊牌、掷骰、宝牌扫光）都登记在
 * runtime 上的数组里，由这里的 animate 统一驱动，避免各处自己起循环。
 */
export async function createRuntime(
  container: HTMLDivElement,
  onDiscard: (tileId: number) => void,
  cameraConfig?: TableCameraConfig,
  tileScale = DEFAULT_TILE_SCALE,
  tileWidthRatio = TILE_WIDTH_RATIO,
  tableclothPath = DEFAULT_TABLECLOTH_ASSET,
): Promise<TableRuntime> {
  const renderer = new THREE.WebGLRenderer({
    antialias: true,
    alpha: true,
    powerPreference: "high-performance",
  });
  renderer.setClearColor(0x000000, 0);
  renderer.outputColorSpace = THREE.SRGBColorSpace;
  renderer.shadowMap.enabled = false;
  renderer.domElement.className = "match-table-canvas__surface";
  container.appendChild(renderer.domElement);

  const scene = new THREE.Scene();
  scene.background = null;
  const perspectiveCamera = new THREE.PerspectiveCamera(25, 1, 0.1, 600);
  const orthographicCamera = new THREE.OrthographicCamera(
    -8,
    8,
    8,
    -8,
    0.1,
    600,
  );
  const root = new THREE.Group();
  scene.add(root);

  scene.add(new THREE.AmbientLight(0xffffff, 2.25));
  scene.add(new THREE.HemisphereLight(0xfff8e7, 0x5e8078, 1.35));
  const keyLight = new THREE.DirectionalLight(0xfff8df, 1.6);
  keyLight.position.set(-4, 12, 7);
  keyLight.castShadow = false;
  scene.add(keyLight);

  const [textures, tableTexture] = await Promise.all([
    loadTileTextures(renderer),
    loadTableTexture(renderer, tableclothPath),
  ]);
  const runtime: TableRuntime = {
    renderer,
    scene,
    camera: perspectiveCamera,
    perspectiveCamera,
    orthographicCamera,
    root,
    textures,
    tableTexture,
    impactDustTexture: createImpactDustTexture(),
    tileGeometries: new Map(),
    tileGeometryWidthRatio: tileWidthRatio,
    selectable: [],
    hovered: null,
    animations: [],
    tilts: [],
    transients: [],
    selfDraw: null,
    pendingRinshanDraws: new Map(),
    spinners: [],
    doraShine: createDoraShineMaterial(),
    impacts: [],
    shake: null,
    cameraBase: new THREE.Vector3(),
    highlightMaterials: new Map(),
    highlightedTileCode: null,
    dangerTileCodes: new Set<string>(),
    revealAllHands: false,
    dimTsumogiri: false,
    instantDraw: false,
    diceRolls: [],
    lastDiscard: null,
    resizeObserver: null as unknown as ResizeObserver,
    frame: 0,
    disposed: false,
    previousView: null,
    openingKey: null,
    renderedOpeningPhase: null,
    latestView: null,
    centerConsoleMesh: null,
    scoreDifferenceVisible: false,
    scoreDifferenceUntil: 0,
    settlementHandKey: null,
    revealedSettlementSeats: new Set(),
    revealedWinningTileSeats: new Set(),
    pointerHandlers: null,
    cameraOverride: cameraConfig ?? null,
    tileScale,
    tileWidthRatio,
    resize: () => {},
    contextLost: false,
    rebuild: () => {},
    handCutGaps: new Map(),
    handCollapses: new Map(),
  };

  const resize = () => {
    const width = Math.max(1, container.clientWidth);
    const height = Math.max(1, container.clientHeight);
    const pixelRatio = Math.min(window.devicePixelRatio || 1, 2);
    renderer.setPixelRatio(pixelRatio);
    renderer.setSize(width, height, false);
    const aspect = width / height;
    const cameraLayout =
      runtime.cameraOverride ?? tableCameraLayout(aspect);
    const activeCamera =
      cameraLayout.mode === "orthographic"
        ? orthographicCamera
        : perspectiveCamera;
    if (activeCamera === perspectiveCamera) {
      perspectiveCamera.aspect = aspect;
      perspectiveCamera.fov = cameraLayout.fov;
    } else {
      const bounds = orthographicCameraBounds(
        cameraLayout.orthographicSize,
        aspect,
      );
      orthographicCamera.left = bounds.left;
      orthographicCamera.right = bounds.right;
      orthographicCamera.top = bounds.top;
      orthographicCamera.bottom = bounds.bottom;
    }
    runtime.cameraBase.set(0, cameraLayout.y, cameraLayout.z);
    activeCamera.position.copy(runtime.cameraBase);
    activeCamera.lookAt(0, cameraLayout.targetY, cameraLayout.targetZ);
    activeCamera.updateProjectionMatrix();
    runtime.camera = activeCamera;
  };
  runtime.resize = resize;
  const resizeObserver = new ResizeObserver(resize);
  resizeObserver.observe(container);
  runtime.resizeObserver = resizeObserver;
  resize();

  const raycaster = new THREE.Raycaster();
  const pointer = new THREE.Vector2();
  const aim = (event: PointerEvent) => {
    const bounds = clientRectInViewport(renderer.domElement);
    const point = clientPointInViewport(
      renderer.domElement,
      event.clientX,
      event.clientY,
    );
    pointer.x = ((point.x - bounds.left) / bounds.width) * 2 - 1;
    pointer.y = -((point.y - bounds.top) / bounds.height) * 2 + 1;
    raycaster.setFromCamera(pointer, runtime.camera);
  };
  const pickTile = (): THREE.Group | null => {
    const hit = raycaster.intersectObjects(runtime.selectable, false)[0];
    return (hit?.object.userData.tileGroup as THREE.Group | undefined) ?? null;
  };
  const pointsAtConsole = (): boolean =>
    runtime.centerConsoleMesh != null &&
    raycaster.intersectObject(runtime.centerConsoleMesh, false).length > 0;
  const onPointerMove = (event: PointerEvent) => {
    aim(event);
    const next = pickTile();
    const consoleHovered = pointsAtConsole();
    if (runtime.hovered) runtime.hovered.userData.hovered = false;
    runtime.hovered = next;
    if (next) next.userData.hovered = true;
    renderer.domElement.style.cursor =
      next || consoleHovered ? "pointer" : "default";
  };
  const onPointerLeave = () => {
    if (runtime.hovered) runtime.hovered.userData.hovered = false;
    runtime.hovered = null;
    renderer.domElement.style.cursor = "default";
  };
  const onPointerDown = (event: PointerEvent) => {
    aim(event);
    if (pointsAtConsole()) {
      toggleCenterConsolePoints(runtime);
      return;
    }
    const group = pickTile();
    const tileId = group?.userData.tileId;
    if (typeof tileId === "number") onDiscard(tileId);
  };
  renderer.domElement.addEventListener("pointermove", onPointerMove);
  renderer.domElement.addEventListener("pointerleave", onPointerLeave);
  renderer.domElement.addEventListener("pointerdown", onPointerDown);
  runtime.pointerHandlers = {
    move: onPointerMove,
    leave: onPointerLeave,
    down: onPointerDown,
  };

  let lastRenderedAt = 0;
  const animate = (now: number) => {
    if (runtime.disposed || runtime.contextLost) return;
    const hoverMoving = runtime.selectable.some((group) => {
      const tileGroup = group.userData.tileGroup as THREE.Group | undefined;
      if (!tileGroup) return false;
      const target = tileGroup.userData.hovered ? 0.34 : 0;
      return Math.abs((tileGroup.userData.hoverLift ?? 0) - target) > 0.002;
    });
    const fullRate =
      hoverMoving ||
      runtime.animations.length > 0 ||
      runtime.tilts.length > 0 ||
      runtime.transients.length > 0 ||
      runtime.impacts.length > 0 ||
      runtime.shake != null ||
      runtime.diceRolls.length > 0;
    /* 主要动作维持 60 FPS；空闲时的扫光和标记动画用 30 FPS，显著减少常驻 GPU 占用。 */
    const frameInterval = 1000 / (fullRate ? 60 : 30);
    if (lastRenderedAt > 0 && now - lastRenderedAt < frameInterval - 1) {
      runtime.frame = requestAnimationFrame(animate);
      return;
    }
    lastRenderedAt = now;
    for (const group of runtime.selectable) {
      const tileGroup = group.userData.tileGroup as THREE.Group | undefined;
      if (!tileGroup) continue;
      const target = tileGroup.userData.hovered ? 0.34 : 0;
      tileGroup.userData.hoverLift = THREE.MathUtils.lerp(
        tileGroup.userData.hoverLift ?? 0,
        target,
        0.2,
      );
      tileGroup.position.y =
        (tileGroup.userData.baseY ?? 0) + tileGroup.userData.hoverLift;
    }
    runtime.animations = runtime.animations.filter((animation) => {
      const progress = THREE.MathUtils.clamp(
        (now - animation.startedAt) / animation.duration,
        0,
        1,
      );
      const eased = 1 - Math.pow(1 - progress, 3);
      animation.group.position.lerpVectors(animation.start, animation.end, eased);
      animation.group.position.y +=
        Math.sin(progress * Math.PI) * (animation.arcHeight ?? 0);
      if (animation.startRotation && animation.endRotation) {
        animation.group.quaternion
          .copy(animation.startRotation)
          .slerp(animation.endRotation, eased);
      }
      if (animation.startScale != null && animation.endScale != null) {
        animation.group.scale.setScalar(
          THREE.MathUtils.lerp(
            animation.startScale,
            animation.endScale,
            eased,
          ),
        );
      }
      animation.group.userData.baseY = animation.group.position.y;
      return progress < 1;
    });
    runtime.tilts = runtime.tilts.filter((tilt) => {
      const progress = THREE.MathUtils.clamp(
        (now - tilt.startedAt) / tilt.duration,
        0,
        1,
      );
      /* 还没轮到它翻：位置可能正被飞行动画接管，别在这里按住不放。 */
      if (progress <= 0) return true;
      const eased = (tilt.ease ?? settlementFallEase)(progress);
      tilt.object.rotation.x = THREE.MathUtils.lerp(
        tilt.startX,
        tilt.endX,
        eased,
      );
      tilt.group.position.lerpVectors(
        tilt.startPosition,
        tilt.endPosition,
        eased,
      );
      /* 自摸甩出去的那张牌：从高处一路掉下来，落地即止。 */
      if (tilt.arcHeight) {
        tilt.group.position.y += tsumoThrowArc(progress) * tilt.arcHeight;
      }
      tilt.group.userData.baseY = tilt.group.position.y;
      return progress < 1;
    });
    for (const spinner of runtime.spinners) {
      const appearAt = spinner.userData.appearAt ?? 0;
      spinner.visible = now >= appearAt;
      if (!spinner.visible) continue;
      spinner.rotation.y += 0.035;
      spinner.position.y =
        (spinner.userData.baseY ?? 0.48) + Math.sin(now * 0.004) * 0.035;
    }
    advanceDoraShine(runtime.doraShine, now);
    runtime.impacts = advanceTableImpacts(runtime.impacts, now);
    runtime.transients = advanceTransientTiles(runtime, now);
    runtime.shake = advanceCameraShake(runtime, now);
    if (
      runtime.scoreDifferenceVisible &&
      now >= runtime.scoreDifferenceUntil
    ) {
      runtime.scoreDifferenceVisible = false;
      updateCenterConsoleTexture(runtime);
    }
    runtime.diceRolls = runtime.diceRolls.filter((roll) => {
      const progress = THREE.MathUtils.clamp(
        (now - roll.startedAt) / roll.duration,
        0,
        1,
      );
      if (progress <= 0) return true;
      const eased = 1 - Math.pow(1 - progress, 3);
      roll.object.position.lerpVectors(
        roll.startPosition,
        roll.endPosition,
        eased,
      );
      roll.object.position.y +=
        Math.sin(progress * Math.PI) * 0.72 +
        Math.abs(Math.sin(progress * Math.PI * 3)) *
          0.16 *
          (1 - progress);
      roll.object.rotation.set(
        THREE.MathUtils.lerp(roll.startEuler.x, roll.endEuler.x, eased),
        THREE.MathUtils.lerp(roll.startEuler.y, roll.endEuler.y, eased),
        THREE.MathUtils.lerp(roll.startEuler.z, roll.endEuler.z, eased),
      );
      if (progress >= 1) {
        roll.object.position.copy(roll.endPosition);
        roll.object.quaternion.copy(roll.finalRotation);
      }
      return progress < 1;
    });
    renderer.render(scene, runtime.camera);
    runtime.frame = requestAnimationFrame(animate);
  };
  /*
   * 显卡随时可能把上下文收走：一个页面同时活着的 WebGL 上下文只有十几个，再开
   * 新的就会把最早那个挤掉，驱动出问题或者显存吃紧也会丢。丢掉的那一瞬间已经排
   * 出去的那一帧还是会照常去链接着色器程序，控制台里于是跳出 `VALIDATE_STATUS
   * false`、着色器日志却是空的那条报错——那不是着色器写错了，是上下文没了。
   *
   * 所以这里把循环停住，等浏览器把上下文还回来再重新量一次画布、整张桌子重画。
   * 不接这两个事件的话，牌桌就只剩一块黑画布，玩家只能刷新。
   */
  renderer.domElement.addEventListener("webglcontextlost", (event) => {
    event.preventDefault();
    runtime.contextLost = true;
    cancelAnimationFrame(runtime.frame);
  });
  renderer.domElement.addEventListener("webglcontextrestored", () => {
    runtime.contextLost = false;
    if (runtime.disposed) return;
    resize();
    runtime.rebuild();
    runtime.frame = requestAnimationFrame(animate);
  });

  runtime.frame = requestAnimationFrame(animate);
  return runtime;
}

async function loadTileTextures(
  renderer: THREE.WebGLRenderer,
): Promise<Map<string, THREE.Texture>> {
  const maxAnisotropy = renderer.capabilities.getMaxAnisotropy();
  const entries = await Promise.all(
    tileCodes.map(async (code) => {
      const texture = await loadSvgCanvasTexture(
        tileAssetPath(code),
        maxAnisotropy,
      );
      texture.colorSpace = THREE.SRGBColorSpace;
      texture.wrapS = THREE.ClampToEdgeWrapping;
      texture.wrapT = THREE.ClampToEdgeWrapping;
      texture.userData.sharedTileTexture = true;
      return [code, texture] as const;
    }),
  );
  return new Map(entries);
}

async function loadSvgCanvasTexture(
  path: string,
  maxAnisotropy: number,
): Promise<THREE.CanvasTexture> {
  const image = await new Promise<HTMLImageElement>((resolve, reject) => {
    const element = new Image();
    element.decoding = "async";
    element.onload = () => resolve(element);
    element.onerror = () =>
      reject(new Error(`无法加载麻将牌矢量图：${path}`));
    element.src = path;
  });
  const canvas = document.createElement("canvas");
  canvas.width = 304;
  canvas.height = 416;
  const context = canvas.getContext("2d");
  if (!context) {
    throw new Error("无法创建麻将牌纹理画布");
  }
  context.clearRect(0, 0, canvas.width, canvas.height);
  context.drawImage(image, 0, 0, canvas.width, canvas.height);
  const texture = new THREE.CanvasTexture(canvas);
  texture.anisotropy = maxAnisotropy;
  return texture;
}

async function loadTableTexture(
  renderer: THREE.WebGLRenderer,
  tableclothPath: string,
): Promise<THREE.Texture> {
  const texture = await new THREE.TextureLoader().loadAsync(tableclothPath);
  texture.colorSpace = THREE.SRGBColorSpace;
  texture.anisotropy = renderer.capabilities.getMaxAnisotropy();
  texture.wrapS = THREE.ClampToEdgeWrapping;
  texture.wrapT = THREE.ClampToEdgeWrapping;
  texture.userData.sharedTableTexture = true;
  return texture;
}

/** 清掉一整棵子树上自己造的资源；同树复用的对象只释放一次。 */
export function disposeGroup(group: THREE.Group): void {
  const geometries = new Set<THREE.BufferGeometry>();
  const materials = new Set<THREE.Material>();
  const textures = new Set<THREE.Texture>();
  group.traverse((object) => {
    if (!(object instanceof THREE.Mesh)) return;
    if (!object.geometry.userData.sharedTileGeometry) {
      geometries.add(object.geometry);
    }
    const meshMaterials = Array.isArray(object.material)
      ? object.material
      : [object.material];
    for (const material of meshMaterials) {
      if (!material.userData.shared) materials.add(material);
    }
  });
  for (const geometry of geometries) geometry.dispose();
  for (const material of materials) {
    const map = (material as THREE.Material & { map?: THREE.Texture | null })
      .map;
    if (
      map &&
      !map.userData.sharedTileTexture &&
      !map.userData.sharedTableTexture &&
      !map.userData.sharedImpactTexture
    ) {
      textures.add(map);
    }
    material.dispose();
  }
  for (const texture of textures) texture.dispose();
}

export function destroyRuntime(runtime: TableRuntime): void {
  runtime.disposed = true;
  cancelAnimationFrame(runtime.frame);
  runtime.resizeObserver.disconnect();
  const canvas = runtime.renderer.domElement;
  const events = runtime.pointerHandlers;
  if (events) {
    canvas.removeEventListener("pointermove", events.move);
    canvas.removeEventListener("pointerleave", events.leave);
    canvas.removeEventListener("pointerdown", events.down);
  }
  disposeGroup(runtime.root);
  disposeTileGeometries(runtime);
  runtime.doraShine.dispose();
  for (const texture of runtime.textures.values()) texture.dispose();
  runtime.tableTexture.dispose();
  runtime.impactDustTexture.dispose();
  runtime.renderer.dispose();
  /*
   * `dispose` 只放掉 three 自己那些 GPU 资源，上下文本身要等垃圾回收才还。牌桌
   * 进出几次（对局、预览、牌桌设置）就攒下几个，超过浏览器允许的数量之后它会强行
   * 掐掉最早的那个——被掐的很可能正是还在用的那张桌子。这里直接还回去。
   */
  runtime.renderer.forceContextLoss();
  canvas.remove();
}

/** 牌宽设置变化或运行时销毁后，释放不再被场景引用的共用牌几何体。 */
export function disposeTileGeometries(runtime: TableRuntime): void {
  for (const geometrySet of runtime.tileGeometries.values()) {
    geometrySet.upper.dispose();
    geometrySet.lower.dispose();
    geometrySet.seam.dispose();
    geometrySet.artwork?.dispose();
  }
  runtime.tileGeometries.clear();
}
