import {
  type MouseEvent as ReactMouseEvent,
  type PointerEvent as ReactPointerEvent,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import type { DiscardWaitHint, MatchView, WaitingTileView } from "../types";
import type { OpeningPhase } from "./OpeningSequence";
import {
  canLocalPlayerDiscard,
  DRAW_MOVE_MS,
  isJokerTile,
  openingDealArrival,
  sortHandForDisplay,
  TILE_STAND_UP_MS,
} from "./table";
import { isDoraTile, tileAssetPath } from "./tileAssets";
import { visibleTileCounts } from "./tileCounts";
import { WaitingTilesPanel } from "./WaitingTilesPanel";

/** 换局之后等开局动画接手的最长时限，超过就自己把手牌放出来。 */
const HAND_START_GRACE_MS = 1500;

interface PlayerHand2DProps {
  view: MatchView;
  openingPhase: OpeningPhase;
  onTileDiscard: (tileId: number) => void;
  riichiSelecting: boolean;
  autoSort?: boolean;
  tileScale?: number;
  /**
   * 摸到的牌直接摆在手上，不给牌山飞过来那一段留空。牌谱重演专用，对局中不传。
   */
  instantDraw?: boolean;
  /** 手牌焦点改变时，只命令三维牌桌更新同种牌材质，不触发场景 React 重绘。 */
  onFocusedTileChange?: (code: string | null) => void;
  /** 有浮层正在播（例如冲击麻将的杠点），播完之前手牌一律点不动。 */
  blocked?: boolean;
}

export function PlayerHand2D({
  view,
  openingPhase,
  onTileDiscard,
  riichiSelecting,
  autoSort = true,
  tileScale = 1,
  instantDraw = false,
  onFocusedTileChange,
  blocked = false,
}: PlayerHand2DProps) {
  const [pendingTileId, setPendingTileId] = useState<number | null>(null);
  const [touchSelectedTileId, setTouchSelectedTileId] =
    useState<number | null>(null);
  const [manualTileOrder, setManualTileOrder] = useState<number[]>([]);
  const [visibleDealCount, setVisibleDealCount] = useState(
    openingPhase === "dice" || openingPhase === "deal"
      ? 0
      : Number.POSITIVE_INFINITY,
  );
  const [waitsVisible, setWaitsVisible] = useState(false);
  const [riichiPreviewTileId, setRiichiPreviewTileId] =
    useState<number | null>(null);
  const [hoveredTileId, setHoveredTileId] = useState<number | null>(null);
  /* 刚摸的那张牌还在牌山到手边的半路上，手上先不摆出来。 */
  const [landing, setLanding] = useState<{
    tileId: number;
    draw: number;
  } | null>(null);
  /* 新一局的牌已经到了，开局动画还没接上，这中间手牌一张都不露。 */
  const [handStartPending, setHandStartPending] = useState(false);
  const startedHandIndex = useRef(view.hand_index);
  const lastDrawnTileId = useRef<number | null | undefined>(undefined);
  /* 牌 ID 每局从头排，隔局摸到同一张也得当作新的一次，飞行计时才不会串。 */
  const drawCount = useRef(0);
  /* 缓存最近一次听牌，非自己回合时仍能显示感叹号。 */
  const cachedWaitsRef = useRef<WaitingTileView[]>([]);
  const lastHandIndex = useRef(view.hand_index);
  const player = view.players.find(
    (candidate) => candidate.seat === view.observer_seat,
  );
  /* 手牌变了就把缓存清掉——不在自己回合就拿不到新听牌，旧数据等下次再算。 */
  const concealedTilesKey = useRef<string>("");
  const currentConcealedKey = useMemo(
    () =>
      (player?.concealed_tiles ?? [])
        .map((tile) => tile.id)
        .sort()
        .join(","),
    [player?.concealed_tiles],
  );
  if (currentConcealedKey !== concealedTilesKey.current) {
    concealedTilesKey.current = currentConcealedKey;
    // 手牌换了，上一次不算了——等下回有数据再存。
    if (cachedWaitsRef.current.length > 0) {
      cachedWaitsRef.current = [];
    }
  }
  const waitHoldTimer = useRef<number | null>(null);
  const onFocusedTileChangeRef = useRef(onFocusedTileChange);
  onFocusedTileChangeRef.current = onFocusedTileChange;
  const tileElements = useRef(new Map<number, HTMLButtonElement>());
  const tilePositions = useRef(new Map<number, number>());
  const previousPhase = useRef(openingPhase);
  const previousAutoSort = useRef(autoSort);
  const lastPointerType = useRef("");
  const suppressClick = useRef(false);
  const dragState = useRef<{
    pointerId: number;
    tileId: number;
    startX: number;
    startY: number;
    dragging: boolean;
  } | null>(null);
  const sortedTiles = useMemo(
    () =>
      sortHandForDisplay(
        player?.concealed_tiles ?? [],
        player?.drawn_tile_id ?? null,
        view.joker_code,
      ),
    [player?.concealed_tiles, player?.drawn_tile_id, view.joker_code],
  );

  /*
   * 摸上来的那张牌在三维里是从牌山飞过来的，飞完才归二维手牌接手。牌还在半空时
   * 手上就多出一张的话，同一张牌会同时出现在两个地方。
   *
   * 这一笔得在渲染里就地记：等画完了再藏，那张牌会先摆出来一帧、整排手牌跟着弹
   * 回去一次——摸一张闪一下就是这么闪的。
   *
   * 只拦刚变的那一次：刷新重进时手上本来就有一张摸牌，那张不该再消失一次。
   * 牌谱重演（`instantDraw`）整段跳过：那边一步就是一个状态，用户还能往回退、
   * 能拖进度条，留这半秒空当只会让手牌一步一闪。
   */
  /*
   * 换局那一帧的空当。
   *
   * 新一局的视图先到，`openingPhase` 要等外面那个 effect 跑完才退回 `dice`，中间
   * 夹着整整一帧：手上已经是新一局的十三张，阶段却还停在上一局的 `play`。于是新
   * 手牌白花花地闪出来（牌面图还没解码，露的是牌面底色），下一帧才随阶段一起消
   * 失，看着就是「先闪一把白牌再瞬间没了」。这里在换局那一刻就把手牌收起来，等
   * 阶段真的退回开局再放。
   */
  const handChanged = startedHandIndex.current !== view.hand_index;
  if (handChanged) {
    startedHandIndex.current = view.hand_index;
    setHandStartPending(true);
  }
  if (handStartPending && openingPhase !== "play") setHandStartPending(false);

  const drawnTileId = player?.drawn_tile_id ?? null;
  if (lastDrawnTileId.current !== drawnTileId) {
    const previous = lastDrawnTileId.current;
    lastDrawnTileId.current = drawnTileId;
    drawCount.current += 1;
    const flying =
      !instantDraw &&
      previous !== undefined &&
      drawnTileId != null &&
      !handChanged &&
      openingPhase === "play";
    setLanding(flying ? { tileId: drawnTileId, draw: drawCount.current } : null);
  }
  const landingTileId = landing?.tileId ?? null;
  const tiles = useMemo(() => {
    if (autoSort) return sortedTiles;
    const byId = new Map(sortedTiles.map((tile) => [tile.id, tile]));
    const ordered = manualTileOrder.flatMap((id) => {
      const tile = byId.get(id);
      return tile ? [tile] : [];
    });
    const known = new Set(ordered.map((tile) => tile.id));
    return [
      ...ordered,
      ...sortedTiles.filter((tile) => !known.has(tile.id)),
    ];
  }, [autoSort, manualTileOrder, sortedTiles]);
  /*
   * 飞行中的那张牌照样摆出来，只是空着（`is-landing`）：格子占住，三维那张飞牌
   * 才有个准地方落——落点就是量这一格量出来的。整格摘掉的话既没处可量，补回来
   * 的那一下还得让整排牌重排一次。
   */
  const visibleTiles =
    openingPhase === "deal" ? tiles.slice(0, visibleDealCount) : tiles;
  const visibleCounts = useMemo(() => visibleTileCounts(view), [view]);
  /* 被鼠标悬停、或者触屏第一下点起来的那张牌：上浮的就是它。 */
  const focusedTileId = hoveredTileId ?? touchSelectedTileId;
  const focusedTileCode =
    visibleTiles.find((tile) => tile.id === focusedTileId)?.code ?? null;
  const riichiHints = view.turn_actions.riichi_discard_hints ?? [];
  const riichiHintKey = riichiHints.map((hint) => hint.tile_id).join(",");
  const riichiPreview =
    riichiHints.find((hint) => hint.tile_id === riichiPreviewTileId) ??
    riichiHints[0];

  /*
   * 开局动画没接上也不能一直藏着。全场有人没加载完时外面那个 effect 会直接返回，
   * 阶段就一直停在 `play`，手牌等不到那次 `dice` ——留一道保险，到点就放出来。
   */
  useEffect(() => {
    if (!handStartPending) return;
    const timer = window.setTimeout(
      () => setHandStartPending(false),
      HAND_START_GRACE_MS,
    );
    return () => window.clearTimeout(timer);
  }, [handStartPending]);

  /* 牌飞到手边、立起来，这张才归手牌摆。 */
  useEffect(() => {
    if (!landing) return;
    const landed = window.setTimeout(
      () => setLanding(null),
      DRAW_MOVE_MS + TILE_STAND_UP_MS,
    );
    return () => window.clearTimeout(landed);
  }, [landing]);

  useEffect(() => {
    if (previousAutoSort.current && !autoSort) {
      setManualTileOrder(sortedTiles.map((tile) => tile.id));
    }
    previousAutoSort.current = autoSort;
  }, [autoSort, sortedTiles]);

  useEffect(() => {
    if (autoSort) return;
    setManualTileOrder((current) =>
      reconcileManualTileOrder(
        current,
        sortedTiles.map((tile) => tile.id),
      ),
    );
  }, [autoSort, sortedTiles]);

  useEffect(() => {
    if (openingPhase === "dice") {
      setVisibleDealCount(0);
      return;
    }
    if (openingPhase !== "deal") {
      setVisibleDealCount(Number.POSITIVE_INFINITY);
      return;
    }
    const seat = view.observer_seat;
    const dealer = view.progress.dealer;
    const seatCount = view.players.length;
    const arrivals = [
      [0, 4],
      [4, 8],
      [8, 12],
      [12, 13],
      [13, 14],
    ] as const;
    const timers = arrivals
      .filter(([tileIndex]) => tileIndex < tiles.length)
      .map(([tileIndex, count]) =>
        window.setTimeout(
          () =>
            setVisibleDealCount((current) =>
              Math.max(current, Math.min(count, tiles.length)),
            ),
          /* 等 3D 那张牌落到手上、翻起来立住，2D 手牌才补上这一格。 */
          openingDealArrival(tileIndex, seat, dealer, seatCount) +
            TILE_STAND_UP_MS,
        ),
      );
    return () => timers.forEach(window.clearTimeout);
  }, [
    openingPhase,
    tiles.length,
    view.hand_index,
    view.observer_seat,
    view.players.length,
    view.progress.dealer,
  ]);

  useEffect(() => {
    if (pendingTileId == null) return;
    const timer = window.setTimeout(() => setPendingTileId(null), 700);
    return () => window.clearTimeout(timer);
  }, [pendingTileId]);

  useEffect(() => {
    setPendingTileId(null);
    setTouchSelectedTileId(null);
    setHoveredTileId(null);
  }, [view.version]);

  useEffect(() => {
    if (autoSort) dragState.current = null;
  }, [autoSort]);

  useEffect(() => {
    if (!riichiSelecting) {
      setRiichiPreviewTileId(null);
      return;
    }
    setRiichiPreviewTileId((current) =>
      current != null &&
      riichiHints.some((hint) => hint.tile_id === current)
        ? current
        : (riichiHints[0]?.tile_id ?? null),
    );
  }, [riichiHintKey, riichiSelecting]);

  useEffect(
    () => () => {
      if (waitHoldTimer.current != null) {
        window.clearTimeout(waitHoldTimer.current);
      }
    },
    [],
  );

  /* 直接命令 Three.js 改材质；父场景不存 hover 状态，避免整排手牌跟着重绘。 */
  useEffect(() => {
    onFocusedTileChange?.(focusedTileCode);
  }, [focusedTileCode, onFocusedTileChange]);

  useEffect(
    () => () => {
      onFocusedTileChangeRef.current?.(null);
    },
    [],
  );

  useLayoutEffect(() => {
    const nextPositions = new Map<number, number>();
    for (const tile of visibleTiles) {
      const element = tileElements.current.get(tile.id);
      if (element) nextPositions.set(tile.id, element.offsetLeft);
    }

    if (
      previousPhase.current === "play" &&
      openingPhase === "play" &&
      tilePositions.current.size > 0
    ) {
      for (const [tileId, nextLeft] of nextPositions) {
        const previousLeft = tilePositions.current.get(tileId);
        const element = tileElements.current.get(tileId);
        if (previousLeft == null || !element) continue;
        const delta = previousLeft - nextLeft;
        if (Math.abs(delta) < 1) continue;
        element.animate(
          [
            { transform: `translateX(${delta}px)` },
            { transform: "translateX(0)" },
          ],
          {
            duration: 300,
            easing: "cubic-bezier(.2,.72,.2,1)",
          },
        );
      }
    }

    tilePositions.current = nextPositions;
    previousPhase.current = openingPhase;
  }, [openingPhase, visibleTiles]);

  if (
    !player ||
    view.phase.kind === "ended" ||
    openingPhase === "dice" ||
    handStartPending
  ) {
    return null;
  }

  const enabled =
    openingPhase === "play" &&
    !blocked &&
    canLocalPlayerDiscard(view) &&
    pendingTileId == null;
  /*
   * 听牌提示只有一块面板，浮在手牌正上方，按这个顺序抢：
   * 立直选牌 → 按住感叹号看自己现在听什么 → 摸到/点到某张牌时预览打出去的听牌。
   */
  const tenpaiHints = view.turn_actions.tenpai_discard_hints ?? [];
  const currentWaits = currentWaitingTiles(
    player.waiting_tiles ?? [],
    tenpaiHints,
    player.drawn_tile_id ?? null,
  );
  /* 非自己回合时服务端不下发 waiting_tiles（冲击麻将尤甚），
     把最近一次算出来的听牌存下来，感叹号就不会在人家的回合里消失。 */
  if (view.hand_index !== lastHandIndex.current) {
    lastHandIndex.current = view.hand_index;
    cachedWaitsRef.current = [];
  }
  if (currentWaits.length > 0) {
    cachedWaitsRef.current = currentWaits;
  }
  const showableWaits =
    currentWaits.length > 0 ? currentWaits : cachedWaitsRef.current;
  const focusedHint =
    enabled && !riichiSelecting && focusedTileId != null
      ? tenpaiHints.find((hint) => hint.tile_id === focusedTileId)
      : undefined;
  const activePanel: { tiles: WaitingTileView[]; preview: boolean } | null =
    riichiSelecting && riichiPreview
      ? { tiles: riichiPreview.waiting_tiles, preview: true }
      : waitsVisible && showableWaits.length > 0
        ? { tiles: showableWaits, preview: false }
        : focusedHint
          ? { tiles: focusedHint.waiting_tiles, preview: true }
          : null;
  const beginWaitHold = () => {
    if (waitHoldTimer.current != null) {
      window.clearTimeout(waitHoldTimer.current);
    }
    waitHoldTimer.current = window.setTimeout(() => {
      setWaitsVisible(true);
      waitHoldTimer.current = null;
    }, 320);
  };
  const endWaitHold = () => {
    if (waitHoldTimer.current != null) {
      window.clearTimeout(waitHoldTimer.current);
      waitHoldTimer.current = null;
    }
    setWaitsVisible(false);
  };
  const discardTile = (tileId: number) => {
    setTouchSelectedTileId(null);
    setPendingTileId(tileId);
    onTileDiscard(tileId);
  };
  const beginTilePointer = (
    event: ReactPointerEvent<HTMLButtonElement>,
    tileId: number,
  ) => {
    lastPointerType.current = event.pointerType;
    suppressClick.current = false;
    if (autoSort || riichiSelecting) return;
    dragState.current = {
      pointerId: event.pointerId,
      tileId,
      startX: event.clientX,
      startY: event.clientY,
      dragging: false,
    };
    event.currentTarget.setPointerCapture(event.pointerId);
  };
  const moveTilePointer = (
    event: ReactPointerEvent<HTMLButtonElement>,
  ) => {
    const drag = dragState.current;
    if (!drag || drag.pointerId !== event.pointerId) return;
    if (
      !drag.dragging &&
      Math.hypot(
        event.clientX - drag.startX,
        event.clientY - drag.startY,
      ) < 7
    ) {
      return;
    }
    drag.dragging = true;
    suppressClick.current = true;
    setTouchSelectedTileId(null);
    event.preventDefault();

    const target = document
      .elementFromPoint(event.clientX, event.clientY)
      ?.closest<HTMLButtonElement>("[data-hand-tile-id]");
    const targetId = Number(target?.dataset.handTileId);
    if (!Number.isInteger(targetId) || targetId === drag.tileId) return;
    setManualTileOrder((current) =>
      moveTileInOrder(current, drag.tileId, targetId),
    );
  };
  const endTilePointer = (
    event: ReactPointerEvent<HTMLButtonElement>,
    tileId: number,
    tileEnabled: boolean,
  ) => {
    const drag = dragState.current;
    const wasDragging =
      drag?.pointerId === event.pointerId && drag.dragging;
    dragState.current = null;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
    if (wasDragging) {
      suppressClick.current = true;
      return;
    }
    if (event.pointerType !== "touch" || !tileEnabled) return;
    if (touchSelectedTileId === tileId) {
      discardTile(tileId);
    } else {
      setTouchSelectedTileId(tileId);
    }
  };
  const clickTile = (
    event: ReactMouseEvent<HTMLButtonElement>,
    tileId: number,
    tileEnabled: boolean,
  ) => {
    if (suppressClick.current) {
      suppressClick.current = false;
      return;
    }
    if (!tileEnabled) return;
    if (lastPointerType.current === "touch" && event.detail > 0) return;
    discardTile(tileId);
  };

  return (
    <div
      className={`match-hand-2d${autoSort ? "" : " is-manual-sort"}`}
      aria-label="我的手牌"
      style={{
        transform: `translateX(-50%) scale(${tileScale})`,
      }}
    >
      {player.furiten && (
        <strong className="match-hand-2d__furiten">振听</strong>
      )}
      {visibleTiles.map((tile) => {
        const drawn = tile.id === player.drawn_tile_id;
        const dora = isDoraTile(tile.code, view.dora_indicators ?? []);
        /* 冲击麻将的财神：百搭牌，单独高亮一档，和宝牌的金边区分开。 */
        const joker = isJokerTile(tile.code, view.joker_code);
        const canDeclareRiichi =
          view.turn_actions.riichi_discard_tile_ids.includes(tile.id);
        const tileEnabled =
          enabled && (!riichiSelecting || canDeclareRiichi);
        return (
          <button
            key={tile.id}
            ref={(element) => {
              if (element) tileElements.current.set(tile.id, element);
              else tileElements.current.delete(tile.id);
            }}
            type="button"
            data-hand-tile-id={tile.id}
            className={`tile-plate match-hand-2d__tile${
              drawn ? " is-drawn" : ""
            }${dora ? " is-dora" : ""}${joker ? " is-joker" : ""}${
              openingPhase === "deal" ? " is-dealing" : ""
            }${tile.id === landingTileId ? " is-landing" : ""}${
              riichiSelecting && canDeclareRiichi
                ? " is-riichi-choice"
                : ""
            }${
              riichiSelecting && !canDeclareRiichi
                ? " is-riichi-blocked"
                : ""
            }${
              touchSelectedTileId === tile.id
                ? " is-touch-selected"
                : ""
            }${!tileEnabled ? " is-not-discardable" : ""}`}
            aria-disabled={!tileEnabled}
            onPointerDown={(event) =>
              beginTilePointer(event, tile.id)
            }
            onPointerMove={moveTilePointer}
            onPointerUp={(event) =>
              endTilePointer(event, tile.id, tileEnabled)
            }
            onPointerCancel={() => {
              dragState.current = null;
              suppressClick.current = false;
            }}
            onPointerEnter={(event) => {
              if (event.pointerType === "mouse") {
                setTouchSelectedTileId(null);
                setHoveredTileId(tile.id);
              }
              if (riichiSelecting && canDeclareRiichi) {
                setRiichiPreviewTileId(tile.id);
              }
            }}
            onPointerLeave={() => {
              setHoveredTileId((current) =>
                current === tile.id ? null : current,
              );
            }}
            onFocus={() => {
              setHoveredTileId(tile.id);
              if (riichiSelecting && canDeclareRiichi) {
                setRiichiPreviewTileId(tile.id);
              }
            }}
            onBlur={() => {
              setHoveredTileId((current) =>
                current === tile.id ? null : current,
              );
            }}
            onClick={(event) =>
              clickTile(event, tile.id, tileEnabled)
            }
          >
            <span className="tile-plate__body match-hand-2d__body">
              <span className="tile-plate__face match-hand-2d__face">
                <img src={tileAssetPath(tile.code, "jp")} alt="" />
              </span>
            </span>
          </button>
        );
      })}
      {activePanel && (
        <div className="match-wait-assist">
          <WaitingTilesPanel
            waitingTiles={activePanel.tiles}
            preview={activePanel.preview}
            visibleCounts={visibleCounts}
          />
        </div>
      )}
      {showableWaits.length > 0 && (
        <button
          type="button"
          className={`match-wait-assist__button${
            waitsVisible ? " is-open" : ""
          }`}
          aria-label="长按查看听牌"
          aria-expanded={waitsVisible}
          onPointerDown={beginWaitHold}
          onPointerUp={endWaitHold}
          onPointerCancel={endWaitHold}
          onPointerLeave={endWaitHold}
          onKeyDown={(event) => {
            if (event.key === " " || event.key === "Enter") {
              event.preventDefault();
              beginWaitHold();
            }
          }}
          onKeyUp={(event) => {
            if (event.key === " " || event.key === "Enter") {
              endWaitHold();
            }
          }}
          onContextMenu={(event) => event.preventDefault()}
        >
          <span aria-hidden="true">!</span>
        </button>
      )}
    </div>
  );
}

/**
 * 「我现在听什么」。
 *
 * 服务端算听牌要的是十三张的牌型，轮到自己、手上多着一张摸牌时算不出来，
 * 于是感叹号按钮到了自己回合反而消失了——恰好是最想看的那一刻。玩家这时候
 * 问的其实是摸切之后听什么，那正是试打提示里摸牌那一条。
 */
export function currentWaitingTiles(
  waitingTiles: WaitingTileView[],
  tenpaiHints: DiscardWaitHint[],
  drawnTileId: number | null,
): WaitingTileView[] {
  if (waitingTiles.length > 0) return waitingTiles;
  if (drawnTileId == null) return [];
  return (
    tenpaiHints.find((hint) => hint.tile_id === drawnTileId)?.waiting_tiles ??
    []
  );
}

export function reconcileManualTileOrder(
  current: number[],
  available: number[],
): number[] {
  const availableSet = new Set(available);
  const retained = current.filter((id) => availableSet.has(id));
  const retainedSet = new Set(retained);
  return [
    ...retained,
    ...available.filter((id) => !retainedSet.has(id)),
  ];
}

export function moveTileInOrder(
  current: number[],
  movingId: number,
  targetId: number,
): number[] {
  const from = current.indexOf(movingId);
  const to = current.indexOf(targetId);
  if (from < 0 || to < 0 || from === to) return current;
  const next = [...current];
  next.splice(from, 1);
  next.splice(to, 0, movingId);
  return next;
}
