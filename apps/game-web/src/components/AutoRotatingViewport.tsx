import {
  type CSSProperties,
  type ReactNode,
  useLayoutEffect,
  useState,
} from "react";
import { autoRotatingViewportFrame } from "./autoRotatingViewportLayout";

type ViewportStyle = CSSProperties & {
  "--auto-viewport-width": string;
  "--auto-viewport-height": string;
  "--auto-viewport-rotation": string;
};

function currentFrame() {
  return typeof window === "undefined"
    ? autoRotatingViewportFrame(1600, 900)
    : autoRotatingViewportFrame(window.innerWidth, window.innerHeight);
}

/**
 * 承载整个前端的自动方向视口。放在这里的内容（包括 fixed 弹窗）会一起缩放、旋转。
 */
export function AutoRotatingViewport({ children }: { children: ReactNode }) {
  const [frame, setFrame] = useState(currentFrame);

  useLayoutEffect(() => {
    const update = () => setFrame(currentFrame());
    update();
    window.addEventListener("resize", update);
    return () => window.removeEventListener("resize", update);
  }, []);

  return (
    <div className="auto-rotating-viewport-host">
      <div
        className="auto-rotating-viewport"
        data-rotated={frame.rotated}
        style={
          {
            "--auto-viewport-width": `${frame.width}px`,
            "--auto-viewport-height": `${frame.height}px`,
            "--auto-viewport-rotation": frame.rotated ? "90deg" : "0deg",
          } as ViewportStyle
        }
      >
        {children}
      </div>
    </div>
  );
}
