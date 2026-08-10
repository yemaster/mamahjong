import { useEffect, useRef, useState, type CSSProperties, type ReactNode } from "react";
import { matchStageScale } from "./matchStageLayout";

/**
 * 把整块二维对局界面架在设计坐标上：高度固定 `900`，宽度横向撑满窗口，整体按
 * 窗口高度缩放。
 *
 * 三维牌桌自己铺满整个窗口，舞台只负责界面层；舞台本身不吃指针事件，交互仍由
 * 各个界面元素自己决定。
 */
export function MatchStage({ children }: { children: ReactNode }) {
  const rootRef = useRef<HTMLDivElement | null>(null);
  const [scale, setScale] = useState(1);

  useEffect(() => {
    const root = rootRef.current;
    if (!root) return;
    const measure = () => {
      setScale(matchStageScale(root.clientHeight));
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(root);
    return () => observer.disconnect();
  }, []);

  return (
    <div className="match-stage-root" ref={rootRef}>
      <div
        className="match-stage"
        style={{ "--match-stage-scale": scale } as CSSProperties}
      >
        {children}
      </div>
    </div>
  );
}
