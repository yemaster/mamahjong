import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";

export interface DragPosition {
  x: number;
  y: number;
}

/**
 * 让一块浮动面板能被拖着在画面里走。
 *
 * 位置记在设计像素里（`MatchStage` 那套 900 高的基准坐标），可指针位移量给的是
 * 真实屏幕像素——舞台整体套了一层 `transform: scale()`，直接拿位移当坐标用，
 * 窗口一小面板就跑得比手快。所以每次按下时先用元素自己的
 * `getBoundingClientRect().width / offsetWidth` 反算出当前缩放比，位移除掉它再落账。
 *
 * 起始位置不写死：舞台宽度跟着窗口比例变，挂上去之后量一次，横向摆正中、纵向按
 * `bottomMargin` 抬到该在的高度。玩家没动过之前，面板自己的尺寸一变（图标或字体
 * 晚一步加载）就重新摆一次——量早了会偏出去半个身子；动过一次之后就再也不挪了。
 */
export function useDragPosition(bottomMargin = 12) {
  const [position, setPosition] = useState<DragPosition | null>(null);
  const nodeRef = useRef<HTMLElement | null>(null);
  /* 玩家拖过一次之后，位置归玩家管，任何自动摆放都得让路。 */
  const movedRef = useRef(false);
  const dragRef = useRef<{
    pointerId: number;
    startX: number;
    startY: number;
    originX: number;
    originY: number;
    scale: number;
  } | null>(null);
  /* 拖动中用 ref 读当前位置，免得 onPointerDown 因为坐标变化反复重建。 */
  const positionRef = useRef<DragPosition | null>(null);
  positionRef.current = position;

  const bounds = useCallback(() => {
    const node = nodeRef.current;
    const stage = node?.offsetParent as HTMLElement | null;
    if (!node || !stage) return { maxX: 0, maxY: 0 };
    return {
      maxX: Math.max(0, stage.offsetWidth - node.offsetWidth),
      maxY: Math.max(0, stage.offsetHeight - node.offsetHeight),
    };
  }, []);

  /* 横向摆正中、纵向从底边往上抬 `bottomMargin`。舞台宽高本来就是设计像素，直接拿来算。 */
  const place = useCallback(() => {
    if (movedRef.current) return;
    const node = nodeRef.current;
    const stage = node?.offsetParent as HTMLElement | null;
    if (!node || !stage) return;
    setPosition({
      x: Math.max(0, (stage.offsetWidth - node.offsetWidth) / 2),
      y: Math.max(0, stage.offsetHeight - node.offsetHeight - bottomMargin),
    });
  }, [bottomMargin]);

  useLayoutEffect(place, [place]);

  /* 面板自己宽高一变（收起展开、图标晚一步加载）就重新摆正，直到玩家自己拖过为止。 */
  useEffect(() => {
    const node = nodeRef.current;
    if (!node || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(() => place());
    observer.observe(node);
    return () => observer.disconnect();
  }, [place]);

  /* 窗口比例变了就把面板夹回画面里，不然横向拖到边上再拉窄窗口就找不着了。 */
  useEffect(() => {
    const stage = nodeRef.current?.offsetParent as HTMLElement | null;
    if (!stage || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(() => {
      const { maxX, maxY } = bounds();
      setPosition((current) =>
        current
          ? { x: clamp(current.x, 0, maxX), y: clamp(current.y, 0, maxY) }
          : current,
      );
    });
    observer.observe(stage);
    return () => observer.disconnect();
  }, [bounds]);

  const onPointerDown = useCallback((event: React.PointerEvent<HTMLElement>) => {
    /* 只认主键，右键和中键留给浏览器。 */
    if (event.button !== 0) return;
    const node = nodeRef.current;
    const origin = positionRef.current;
    if (!node || !origin) return;
    const rect = node.getBoundingClientRect();
    const scale = node.offsetWidth > 0 ? rect.width / node.offsetWidth : 1;
    dragRef.current = {
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      originX: origin.x,
      originY: origin.y,
      scale: scale > 0 ? scale : 1,
    };
    event.currentTarget.setPointerCapture(event.pointerId);
    event.preventDefault();
  }, []);

  const onPointerMove = useCallback(
    (event: React.PointerEvent<HTMLElement>) => {
      const drag = dragRef.current;
      if (!drag || drag.pointerId !== event.pointerId) return;
      const { maxX, maxY } = bounds();
      const dx = (event.clientX - drag.startX) / drag.scale;
      const dy = (event.clientY - drag.startY) / drag.scale;
      /* 真挪过一下才算数：光按一下把手不该把面板钉死在原地。 */
      movedRef.current = true;
      setPosition({
        x: clamp(drag.originX + dx, 0, maxX),
        y: clamp(drag.originY + dy, 0, maxY),
      });
    },
    [bounds],
  );

  const onPointerUp = useCallback((event: React.PointerEvent<HTMLElement>) => {
    if (dragRef.current?.pointerId !== event.pointerId) return;
    dragRef.current = null;
  }, []);

  return {
    position,
    /** 挂在面板本体上：量缩放比、量边界都靠它。 */
    nodeRef,
    /** 摊到拖动把手上。 */
    handleProps: {
      onPointerDown,
      onPointerMove,
      onPointerUp,
      onPointerCancel: onPointerUp,
    },
  };
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}
