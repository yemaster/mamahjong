import type * as THREE from "three";
import type { MatchView } from "../../types";
import type { OpeningPhase } from "../OpeningSequence";
import type { ExchangeSnapshot, ExchangeState } from "./exchange";

export interface TableCameraConfig {
  mode: "perspective" | "orthographic";
  fov: number;
  orthographicSize: number;
  y: number;
  z: number;
  targetY: number;
  targetZ: number;
}

/** 一张牌从起点滑到终点的补间，牌墙发牌、打牌、推副露都走这里。 */
export interface TileAnimation {
  group: THREE.Group;
  start: THREE.Vector3;
  end: THREE.Vector3;
  startRotation?: THREE.Quaternion;
  endRotation?: THREE.Quaternion;
  startedAt: number;
  duration: number;
  arcHeight?: number;
  /**
   * 飞行途中一并改变的缩放。两个都给才生效。
   *
   * 只有主视角摸的那张牌用得上：它要落到二维手牌那一格上，而二维的牌比三维桌
   * 上的牌大得多，一路长过去才不至于在落地那一刻突然涨一圈。
   */
  startScale?: number;
  endScale?: number;
}

/**
 * 一张正在从手边飞往牌河的牌。
 *
 * 视图可能在飞行动画结束前再次更新（例如其他玩家自动跳过响应）。这份状态不挂在
 * 临时场景节点上，因此牌河层被后续状态替换后仍能从原进度继续，而不是把牌瞬移
 * 到终点。
 */
export interface DiscardFlight {
  startedAt: number;
  start: THREE.Vector3;
  startRotation: THREE.Quaternion;
}

/**
 * 绕 X 轴翻牌：结算时立着的牌倒下（摊牌）或翻扣（盖牌），
 * 以及刚摸到手上的牌从平躺翻起来立住。
 */
export interface TileTiltAnimation {
  object: THREE.Object3D;
  startX: number;
  endX: number;
  startedAt: number;
  duration: number;
  /** true when the tile is being covered (盖牌), false when revealed (摊牌) */
  covering: boolean;
  /** The tile slides while it rotates so the fall reads as an edge hinge. */
  group: THREE.Object3D;
  startPosition: THREE.Vector3;
  endPosition: THREE.Vector3;
  /** 缺省是结算砸下去的曲线；翻起来的牌自己给一条。 */
  ease?: (progress: number) => number;
  /** 被甩出去的牌走一条抛物线，这里是它抬起来的最大高度。 */
  arcHeight?: number;
}

/** 只登台一会儿的牌：到点就地拆掉，不留在桌上。 */
export interface TransientTile {
  group: THREE.Group;
  removeAt: number;
}

export interface DiceRollAnimation {
  object: THREE.Object3D;
  startPosition: THREE.Vector3;
  endPosition: THREE.Vector3;
  startEuler: THREE.Vector3;
  endEuler: THREE.Vector3;
  finalRotation: THREE.Quaternion;
  startedAt: number;
  duration: number;
}

/** 牌砸在桌上扬起的那层灰。散完就地拆掉，不留在场景里。 */
export interface TableImpact {
  mesh: THREE.Mesh;
  material: THREE.MeshBasicMaterial;
  /** 可以排在将来：牌落地那一刻这层灰才开始扬。 */
  startedAt: number;
  duration: number;
}

/** 砸下去那一刻镜头被撞的那一颤。 */
export interface CameraShake {
  startedAt: number;
  duration: number;
  /** 偏出基准位的最大距离，世界坐标。 */
  amplitude: number;
}

/** 一张桌上明牌的牌面白底，连同它自己那份底色，好在取消点亮时还原。 */
export interface HighlightTileFace {
  material: THREE.MeshBasicMaterial;
  base: THREE.Color;
}

/** 同一尺寸的牌共用一组不可变几何体，局部层更新时只换必要的实例和材质。 */
export interface TileGeometrySet {
  upper: THREE.BufferGeometry;
  lower: THREE.BufferGeometry;
  seam: THREE.BufferGeometry;
  artwork: THREE.BufferGeometry | null;
}

export interface TableRuntime {
  renderer: THREE.WebGLRenderer;
  scene: THREE.Scene;
  camera: THREE.Camera;
  perspectiveCamera: THREE.PerspectiveCamera;
  orthographicCamera: THREE.OrthographicCamera;
  root: THREE.Group;
  /** 当前构建目标；正常指向 root，局部更新时临时指向对应的缓存层。 */
  renderTarget: THREE.Group;
  /** 按视觉区域缓存的场景子树，视图更新时只替换签名发生变化的区域。 */
  layers: Map<string, { signature: string; group: THREE.Group }>;
  /** 被新层顶替的旧层；保持隐藏，等新层真正渲染一帧后再统一释放。 */
  pendingLayerDisposals: THREE.Group[];
  textures: Map<string, THREE.Texture>;
  tableTexture: THREE.Texture;
  tableclothPath: string;
  requestedTableclothPath: string;
  /** 砸牌扬尘共用的 Canvas 纹理，避免为短动画编译自定义 shader。 */
  impactDustTexture: THREE.Texture;
  tileGeometries: Map<string, TileGeometrySet>;
  /** 牌身、牌背和牌面图案材质按样式复用，局部图层替换时不重新编译。 */
  tileMaterials: Map<string, THREE.Material>;
  tileGeometryWidthRatio: number;
  selectable: THREE.Mesh[];
  hovered: THREE.Group | null;
  animations: TileAnimation[];
  /** 按「座位:牌 id」保存尚未结束的出牌飞行动画。 */
  discardFlights: Map<string, DiscardFlight>;
  tilts: TileTiltAnimation[];
  spinners: THREE.Object3D[];
  /** 宝牌那道扫光的材质，整张桌子共用一份，动画循环里统一推进相位。 */
  doraShine: THREE.ShaderMaterial;
  /** 桌面上还没散尽的灰。 */
  impacts: TableImpact[];
  /** 演完就得拆掉的牌，比如主视角自己摸上来的那一张。 */
  transients: TransientTile[];
  /**
   * 主视角正在飞的那张摸牌：牌号和起飞时刻。
   *
   * 自摸层可能因结算或显示参数变化被替换，飞到半路的节点会跟着离场，而二维手牌
   * 那一格还空着。记下这一笔，层被替换时照原来的起飞时刻把它接着飞完。
   */
  selfDraw: { tileId: number; takeOffAt: number; wallSlot: number } | null;
  /**
   * 已经完成杠、但岭上牌还没真正出现在视图里的座位。
   *
   * 冲击麻将会先停在杠点动画阶段，等四家播完才补牌；这张表把牌山末尾的来源槽位
   * 跨过那一帧保存下来，避免补牌被误判成普通摸牌而从牌山开头飞出。
   */
  pendingRinshanDraws: Map<number, number>;
  /** 开局发牌时，物理牌山槽位到该张牌起飞时刻的映射。 */
  openingWallTakeoffs: Map<number, number>;
  openingWallTakeoffKey: string | null;
  /** 正在进行的镜头颤动；`null` 表示镜头是稳的。 */
  shake: CameraShake | null;
  /** 相机没被震偏时该待的地方，颤完照这个放回去。 */
  cameraBase: THREE.Vector3;
  /** 桌上摊开的明牌，按牌种归拢，拿起手牌时用来点亮同种牌。 */
  highlightMaterials: Map<string, HighlightTileFace[]>;
  /** 牌河或副露层被替换后才需要重新遍历并建立高亮索引。 */
  highlightIndexDirty: boolean;
  /** 当前被拿起的那种牌；`null` 表示没有。 */
  highlightedTileCode: string | null;
  /** 牌谱重演的铳牌提示：这些牌种在桌上一律染红。对局中永远是空的。 */
  dangerTileCodes: Set<string>;
  /**
   * 牌谱重演的摊牌开关。
   *
   * 为真时别家的手牌正面朝上（照旧立着，不是结算时那样摊平），不看结算状态。
   * 对局中永远是 `false`——牌谱之外没有任何一条路能看见别人的暗手。
   */
  revealAllHands: boolean;
  /**
   * 牌谱重演的摸切压暗。
   *
   * 为真时牌河里摸切的那些牌整体暗一档。对局中永远是 `false`：手切摸切是读牌的
   * 情报，实时对局里没人替你标出来，牌谱回头复盘才给。
   */
  dimTsumogiri: boolean;
  /**
   * 牌谱重演的摸牌：牌直接出现在手上，不从牌山飞过来。
   *
   * 对局中永远是 `false`——摸牌那一下是实时对局的节奏，看得见牌从哪儿来才对得上
   * 牌山的余量。牌谱那边一步一个状态，飞到半路就会被下一步推倒。
   */
  instantDraw: boolean;
  diceRolls: DiceRollAnimation[];
  lastDiscard: { seat: number; index: number } | null;
  resizeObserver: ResizeObserver;
  frame: number;
  disposed: boolean;
  previousView: MatchView | null;
  openingKey: string | null;
  renderedOpeningPhase: OpeningPhase | null;
  latestView: MatchView | null;
  centerConsoleMesh: THREE.Mesh | null;
  scoreDifferenceVisible: boolean;
  scoreDifferenceUntil: number;
  settlementHandKey: string | null;
  revealedSettlementSeats: Set<number>;
  revealedWinningTileSeats: Set<number>;
  /** 四川血战到底：对局中已经盖倒过手牌的胡牌家，重建时不再重放倒牌动画。 */
  coveredWonSeats: Set<number>;
  pointerHandlers: {
    move: (event: PointerEvent) => void;
    leave: () => void;
    down: (event: PointerEvent) => void;
  } | null;
  cameraOverride: TableCameraConfig | null;
  tileScale: number;
  tileWidthRatio: number;
  resize: () => void;
  /** 显卡把上下文收走了：这段时间什么都画不了，等它还回来。 */
  contextLost: boolean;
  /** 上下文还回来之后按最新视图重画一张桌子。 */
  rebuild: () => void;
  /**
   * 手切空隙：key = seat，value = 被切掉那张牌在「切牌前」手牌里的位置.
   *
   * 手切那家立姿牌阵里应该缺那一格，让围观的人肉眼分得出手切摸切。
   * 固定保留一秒后直接移动当前牌节点完成归拢，不重建手牌层。
   */
  handCutGaps: Map<number, HandCutGap>;
  /**
   * 四川麻将换三张的演出状态。`null` 表示不在换牌阶段（或演出已收束）。
   * 飞出、换位、飞入由 `advanceExchange`（rAF 循环）推进，主视角手牌层由它代管。
   */
  exchange: ExchangeState | null;
  /**
   * 提交换牌那一刻抓下的快照。存在 runtime 上而不是随每次渲染传参，演出状态机
   * 在 rAF 循环里随时读得到，不受 React 渲染时机影响。
  */
  exchangeSnapshot: ExchangeSnapshot | null;
  /** 当前 runtime 快照所属的牌局，避免跨局复用 React 尚未清掉的旧状态。 */
  exchangeSnapshotKey: string | null;
  /** 已完成的换牌局标识；防止旧 React 快照回流时把同一段动画重新启动。 */
  exchangeCompletedKey?: string | null;
  /** 换牌临时牌离场后，下一次视图同步必须用权威牌背重建这些手牌层。 */
  forceHandRebuildSeats: Set<number>;
  /** 换三张动画播完的回调：由 GameTable 注入，用于向服务端报告回执。 */
  onExchangeDone: (() => void) | null;
}

/** 手切牌在别家手上留下的空隙。 */
export interface HandCutGap {
  /** 在切牌前手牌（不含摸进那张）里的 0-based 位置。 */
  gapPosition: number;
  /** 被切掉的那张牌的 id，用来阻止旧定时器误清理新状态。 */
  tileId: number;
}
