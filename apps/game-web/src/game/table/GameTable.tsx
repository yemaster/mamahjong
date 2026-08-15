import { forwardRef, useEffect, useImperativeHandle, useRef } from "react";
import type { MatchView } from "../../types";
import type { OpeningPhase } from "../OpeningSequence";
import {
  DEFAULT_TABLECLOTH_ASSET,
  DEFAULT_TILE_SCALE,
  TILE_WIDTH_RATIO,
} from "./constants";
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
    onRendererError,
    dangerTileCodes,
    revealAllHands = false,
    dimTsumogiri = false,
    instantDraw = false,
    cameraConfig,
    tileScale = DEFAULT_TILE_SCALE,
    tileWidthRatio = TILE_WIDTH_RATIO,
    tableclothPath = DEFAULT_TABLECLOTH_ASSET,
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
  const latestRenderRef = useRef({
    view,
    openingPhase,
    dice,
    settlementRevealSeats,
    settlementWinningTileSeats,
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
  latestRenderRef.current = {
    view,
    openingPhase,
    dice,
    settlementRevealSeats,
    settlementWinningTileSeats,
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
        /* 上下文恢复或局部动画定时器触发时，按最新视图做一次增量同步。 */
        runtime.rebuild = () => {
          runtime.openingKey = null;
          runtime.renderedOpeningPhase = null;
          runtime.revealAllHands = revealAllHandsRef.current;
          runtime.dimTsumogiri = dimTsumogiriRef.current;
          runtime.instantDraw = instantDrawRef.current;
          const current = latestRenderRef.current;
          renderTable(
            runtime,
            current.view,
            current.openingPhase,
            current.dice,
            current.settlementRevealSeats,
            current.settlementWinningTileSeats,
          );
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
        renderTable(
          currentRuntime,
          current.view,
          current.openingPhase,
          current.dice,
          current.settlementRevealSeats,
          current.settlementWinningTileSeats,
        );
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
  ]);

  return (
    <div
      ref={containerRef}
      className="match-table-canvas match-table-canvas--three"
      aria-label="三维麻将牌桌"
    />
  );
});
