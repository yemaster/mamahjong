import { forwardRef, useEffect, useImperativeHandle, useRef } from "react";
import type { MatchView } from "../../types";
import type { OpeningPhase } from "../OpeningSequence";
import {
  DEFAULT_TABLECLOTH_ASSET,
  DEFAULT_TILE_SCALE,
  TILE_WIDTH_RATIO,
} from "./constants";
import type { ExchangeSnapshot } from "./exchange";
import {
  createRuntime,
  destroyRuntime,
  updateRuntimeTablecloth,
} from "./runtime";
import { renderTable } from "./scene";
import { setTableDangerTiles, setTableTileHighlight } from "./tileHighlight";
import type { TableCameraConfig, TableRuntime } from "./types";

export interface GameTableHandle {
  setFocusedTileCode: (code: string | null) => void;
}

interface GameTableProps {
  view: MatchView;
  openingPhase: OpeningPhase;
  dice: [number, number];
  onTileDiscard: (tileId: number) => void;
  settlementRevealSeats?: number[];
  /** Seats whose 自摸 tile is flipped up ahead of the rest of the hand. */
  settlementWinningTileSeats?: number[];
  /** 四川血战中，点数动画后允许哪一家进入盖牌/亮胡张动画。 */
  sichuanWinRevealSeats?: number[];
  onRendererError?: () => void;
  /** 牌谱重演的铳牌提示：这些牌种在桌上染红。对局中不传。 */
  dangerTileCodes?: string[];
  /** 牌谱重演的摊牌开关：为真时别人的暗手正面朝上，不看结算状态。 */
  revealAllHands?: boolean;
  /** 牌谱重演的摸切压暗：为真时牌河里摸切的牌整体暗一档。对局中不传。 */
  dimTsumogiri?: boolean;
  /** 牌谱重演的摸牌：为真时摸到的牌直接在手上，不从牌山飞。对局中不传。 */
  instantDraw?: boolean;
  cameraConfig?: TableCameraConfig;
  tileScale?: number;
  tileWidthRatio?: number;
  tableclothPath?: string;
  /**
   * 四川换三张：提交那一刻抓下的换前手牌快照。`null` 表示还没提交，
   * 演出模块照着它播飞出/换位/飞入。
   */
  exchangeSnapshot?: ExchangeSnapshot | null;
  /** 四川换三张：整段换牌动画播完时回调，用于向服务端报告回执。 */
  onExchangeAnimationDone?: () => void;
}

/**
 * 三维牌桌。
 *
 * React 只负责挂载画布和把最新的视图交给 {@link renderTable}，桌面本身是一棵
 * 命令式的 three.js 场景树，活在 {@link TableRuntime} 上。
 */
export const GameTable = forwardRef<GameTableHandle, GameTableProps>(function GameTable(
  {
    view,
    openingPhase,
    dice,
    onTileDiscard,
    settlementRevealSeats = [],
    settlementWinningTileSeats = [],
    sichuanWinRevealSeats = [],
    onRendererError,
    dangerTileCodes,
    revealAllHands = false,
    dimTsumogiri = false,
    instantDraw = false,
    cameraConfig,
    tileScale = DEFAULT_TILE_SCALE,
    tileWidthRatio = TILE_WIDTH_RATIO,
    tableclothPath = DEFAULT_TABLECLOTH_ASSET,
    exchangeSnapshot = null,
    onExchangeAnimationDone,
  }: GameTableProps,
  ref,
) {
  const containerRef = useRef<HTMLDivElement>(null);
  const runtimeRef = useRef<TableRuntime | null>(null);
  const renderFrameRef = useRef<number | null>(null);
  const discardRef = useRef(onTileDiscard);
  const rendererErrorRef = useRef(onRendererError);
  const cameraConfigRef = useRef(cameraConfig);
  const tileScaleRef = useRef(tileScale);
  const tileWidthRatioRef = useRef(tileWidthRatio);
  const tableclothPathRef = useRef(tableclothPath);
  const focusedTileCodeRef = useRef<string | null>(null);
  const dangerTileCodesRef = useRef(dangerTileCodes);
  const revealAllHandsRef = useRef(revealAllHands);
  const dimTsumogiriRef = useRef(dimTsumogiri);
  const instantDrawRef = useRef(instantDraw);
  const exchangeDoneRef = useRef(onExchangeAnimationDone);
  const latestRenderRef = useRef({
    view,
    openingPhase,
    dice,
    settlementRevealSeats,
    settlementWinningTileSeats,
    sichuanWinRevealSeats,
    exchangeSnapshot,
  });

  discardRef.current = onTileDiscard;
  rendererErrorRef.current = onRendererError;
  cameraConfigRef.current = cameraConfig;
  tileScaleRef.current = tileScale;
  tileWidthRatioRef.current = tileWidthRatio;
  tableclothPathRef.current = tableclothPath;
  dangerTileCodesRef.current = dangerTileCodes;
  revealAllHandsRef.current = revealAllHands;
  dimTsumogiriRef.current = dimTsumogiri;
  instantDrawRef.current = instantDraw;
  exchangeDoneRef.current = onExchangeAnimationDone;
  latestRenderRef.current = {
    view,
    openingPhase,
    dice,
    settlementRevealSeats,
    settlementWinningTileSeats,
    sichuanWinRevealSeats,
    exchangeSnapshot,
  };

  useImperativeHandle(
    ref,
    () => ({
      setFocusedTileCode(code) {
        focusedTileCodeRef.current = code;
        const runtime = runtimeRef.current;
        if (runtime) setTableTileHighlight(runtime, code);
      },
    }),
    [],
  );

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    let cancelled = false;
    void createRuntime(
      container,
      (tileId) => discardRef.current(tileId),
      cameraConfig,
      tileScale,
      tileWidthRatio,
      tableclothPath,
    )
      .then((runtime) => {
        if (cancelled) {
          destroyRuntime(runtime);
          return;
        }
        runtime.cameraOverride = cameraConfigRef.current ?? null;
        runtime.tileScale = tileScaleRef.current;
        runtime.tileWidthRatio = tileWidthRatioRef.current;
        runtime.resize();
        runtimeRef.current = runtime;
        /* 换三张动画播完的回执走 React 注入的回调，这里始终读最新一份。 */
        runtime.onExchangeDone = () => exchangeDoneRef.current?.();
        /* 上下文恢复或局部动画定时器触发时，按最新视图做一次增量同步。 */
        runtime.rebuild = () => {
          runtime.openingKey = null;
          runtime.renderedOpeningPhase = null;
          runtime.revealAllHands = revealAllHandsRef.current;
          runtime.dimTsumogiri = dimTsumogiriRef.current;
          runtime.instantDraw = instantDrawRef.current;
          const current = latestRenderRef.current;
          if (current.sichuanWinRevealSeats.length > 0) {
            renderTable(
              runtime,
              current.view,
              current.openingPhase,
              current.dice,
              current.settlementRevealSeats,
              current.settlementWinningTileSeats,
              current.exchangeSnapshot,
              current.sichuanWinRevealSeats,
            );
          } else {
            renderTable(
              runtime,
              current.view,
              current.openingPhase,
              current.dice,
              current.settlementRevealSeats,
              current.settlementWinningTileSeats,
              current.exchangeSnapshot,
            );
          }
          setTableTileHighlight(runtime, focusedTileCodeRef.current);
          setTableDangerTiles(runtime, dangerTileCodesRef.current ?? []);
        };
        runtime.rebuild();
        if (runtime.tableclothPath !== tableclothPathRef.current) {
          void updateRuntimeTablecloth(runtime, tableclothPathRef.current).catch(
            () => rendererErrorRef.current?.(),
          );
        }
      })
      .catch(() => {
        if (!cancelled) rendererErrorRef.current?.();
      });

    return () => {
      cancelled = true;
      if (renderFrameRef.current != null) {
        window.cancelAnimationFrame(renderFrameRef.current);
        renderFrameRef.current = null;
      }
      if (runtimeRef.current) {
        destroyRuntime(runtimeRef.current);
        runtimeRef.current = null;
      }
    };
  }, [view.id]);

  useEffect(() => {
    const runtime = runtimeRef.current;
    if (!runtime || runtime.tableclothPath === tableclothPath) return;
    void updateRuntimeTablecloth(runtime, tableclothPath).catch(() =>
      rendererErrorRef.current?.(),
    );
  }, [tableclothPath]);

  /* 铳牌的红只改材质颜色，不重建场景。 */
  useEffect(() => {
    const runtime = runtimeRef.current;
    if (!runtime) return;
    setTableDangerTiles(runtime, dangerTileCodes ?? []);
  }, [(dangerTileCodes ?? []).join(",")]);

  useEffect(() => {
    const runtime = runtimeRef.current;
    if (!runtime) return;
    runtime.cameraOverride = cameraConfig ?? null;
    runtime.resize();
  }, [
    cameraConfig?.fov,
    cameraConfig?.mode,
    cameraConfig?.orthographicSize,
    cameraConfig?.targetY,
    cameraConfig?.targetZ,
    cameraConfig?.y,
    cameraConfig?.z,
  ]);

  useEffect(() => {
    const runtime = runtimeRef.current;
    if (!runtime) return;
    runtime.tileScale = tileScale;
    runtime.tileWidthRatio = tileWidthRatio;
    runtime.revealAllHands = revealAllHands;
    runtime.dimTsumogiri = dimTsumogiri;
    runtime.instantDraw = instantDraw;
    /*
     * React / WebSocket 可能在一个屏幕帧内连续提交多份状态（出牌、无人响应、下家
     * 摸牌）。只记录最新请求，并在下一次 rAF 同步一次 Three.js；React 重渲染本身
     * 不再直接触发场景提交。
     */
    if (renderFrameRef.current == null) {
      renderFrameRef.current = window.requestAnimationFrame(() => {
        renderFrameRef.current = null;
        const currentRuntime = runtimeRef.current;
        if (!currentRuntime || currentRuntime.disposed) return;
        const current = latestRenderRef.current;
        currentRuntime.tileScale = tileScaleRef.current;
        currentRuntime.tileWidthRatio = tileWidthRatioRef.current;
        currentRuntime.revealAllHands = revealAllHandsRef.current;
        currentRuntime.dimTsumogiri = dimTsumogiriRef.current;
        currentRuntime.instantDraw = instantDrawRef.current;
        if (current.sichuanWinRevealSeats.length > 0) {
          renderTable(
            currentRuntime,
            current.view,
            current.openingPhase,
            current.dice,
            current.settlementRevealSeats,
            current.settlementWinningTileSeats,
            current.exchangeSnapshot,
            current.sichuanWinRevealSeats,
          );
        } else {
          renderTable(
            currentRuntime,
            current.view,
            current.openingPhase,
            current.dice,
            current.settlementRevealSeats,
            current.settlementWinningTileSeats,
            current.exchangeSnapshot,
          );
        }
      });
    }
  }, [
    dice[0],
    dice[1],
    openingPhase,
    view.hand_index,
    view.id,
    view.version,
    revealAllHands,
    dimTsumogiri,
    instantDraw,
    tileScale,
    tileWidthRatio,
    settlementRevealSeats.join(","),
    settlementWinningTileSeats.join(","),
    sichuanWinRevealSeats.join(","),
    exchangeSnapshot,
  ]);

  return (
    <div
      ref={containerRef}
      className="match-table-canvas match-table-canvas--three"
      aria-label="三维麻将牌桌"
    />
  );
});
