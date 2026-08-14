interface Point {
  x: number;
  y: number;
}

export interface ViewportClientRect {
  left: number;
  top: number;
  width: number;
  height: number;
}

const VIEWPORT_SELECTOR = ".auto-rotating-viewport";

function viewportFor(element: Element): HTMLElement | null {
  return element.closest<HTMLElement>(VIEWPORT_SELECTOR);
}

function isRotated(viewport: HTMLElement): boolean {
  return viewport.dataset.rotated === "true";
}

/** 把屏幕上的指针坐标还原到未旋转的应用坐标平面。 */
export function clientPointInViewport(
  element: Element,
  clientX: number,
  clientY: number,
): Point {
  const viewport = viewportFor(element);
  if (!viewport) return { x: clientX, y: clientY };

  const bounds = viewport.getBoundingClientRect();
  if (!isRotated(viewport)) {
    return { x: clientX - bounds.left, y: clientY - bounds.top };
  }

  /*
   * 视口顺时针 90° 后：logical(x, y) -> screen(right - y, top + x)。
   * 这里应用逆变换，让 Three.js 射线、拖动等继续使用原来的横向设计坐标。
   */
  return {
    x: clientY - bounds.top,
    y: bounds.right - clientX,
  };
}
/** 把屏幕位移向量还原到未旋转的应用坐标轴。 */
export function clientVectorInViewport(
  element: Element,
  deltaX: number,
  deltaY: number,
): Point {
  const viewport = viewportFor(element);
  if (!viewport || !isRotated(viewport)) {
    return { x: deltaX, y: deltaY };
  }
  return { x: deltaY, y: -deltaX };
}

/**
 * getBoundingClientRect() 在整页旋转后会交换宽高；把它转回应用坐标，供缩放与
 * Three.js 屏幕投影计算使用。
 */
export function clientRectInViewport(element: Element): ViewportClientRect {
  const rect = element.getBoundingClientRect();
  const viewport = viewportFor(element);
  if (!viewport) {
    return {
      left: rect.left,
      top: rect.top,
      width: rect.width,
      height: rect.height,
    };
  }

  const points = [
    clientPointInViewport(element, rect.left, rect.top),
    clientPointInViewport(element, rect.right, rect.top),
    clientPointInViewport(element, rect.left, rect.bottom),
    clientPointInViewport(element, rect.right, rect.bottom),
  ];
  const xs = points.map((point) => point.x);
  const ys = points.map((point) => point.y);
  const left = Math.min(...xs);
  const top = Math.min(...ys);

  return {
    left,
    top,
    width: Math.max(...xs) - left,
    height: Math.max(...ys) - top,
  };
}
