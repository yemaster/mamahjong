import {
  type CSSProperties,
  type ReactNode,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import {
  DOM_STAGE_HEIGHT,
  DOM_STAGE_WIDTH,
  fixedDomStageFrame,
} from "./fixedDomStageLayout";

type StageStyle = CSSProperties & {
  "--dom-stage-scale": number;
  "--dom-stage-left": string;
  "--dom-stage-top": string;
};

/**
 * 固定 1600×900 的 DOM 设计舞台。
 *
 * 不同屏幕只改变最外层的统一缩放倍率；内部布局、坐标和断点均保持不变。
 */
export function FixedDomStage({
  children,
  variant = "lobby",
}: {
  children: ReactNode;
  variant?: "splash" | "lobby" | "transition";
}) {
  const stageRef = useRef<HTMLDivElement | null>(null);
  const [frame, setFrame] = useState(() =>
    typeof window === "undefined"
      ? fixedDomStageFrame(DOM_STAGE_WIDTH, DOM_STAGE_HEIGHT)
      : fixedDomStageFrame(window.innerWidth, window.innerHeight),
  );

  useLayoutEffect(() => {
    const stage = stageRef.current;
    if (!stage) return;
    const update = () => {
      setFrame(fixedDomStageFrame(stage.clientWidth, stage.clientHeight));
    };
    update();
    if (typeof ResizeObserver !== "undefined") {
      const observer = new ResizeObserver(update);
      observer.observe(stage);
      return () => observer.disconnect();
    }
    /* 旧版 WebView 没有 ResizeObserver 时仍保证旋转、改尺寸后会重算。 */
    window.addEventListener("resize", update);
    return () => window.removeEventListener("resize", update);
  }, []);

  return (
    <div
      ref={stageRef}
      className={`fixed-dom-stage fixed-dom-stage--${variant}`}
      style={
        {
          "--dom-stage-scale": frame.scale,
          "--dom-stage-left": `${frame.left}px`,
          "--dom-stage-top": `${frame.top}px`,
        } as StageStyle
      }
    >
      <div
        className="fixed-dom-stage__content"
        data-design-width={DOM_STAGE_WIDTH}
        data-design-height={DOM_STAGE_HEIGHT}
      >
        {children}
      </div>
    </div>
  );
}
