import type { CSSProperties } from "react";
import { tileAssetPath } from "./tileAssets";

/**
 * 主视角手牌那套实体牌：绿色牌背层、灰色过渡层、象牙白牌面，最后叠透明 SVG。
 *
 * 手牌、听牌提示和流局摊牌横幅共用这一个模型，缩放只靠 `--tile-plate-width`
 * 和 `--tile-plate-lift` 两个变量，不再各写一份牌面样式。
 */
export function TilePlate({
  code,
  width,
  lift,
  className = "",
}: {
  code: string;
  /** 牌宽；不给就沿用所在容器上的 `--tile-plate-width`。 */
  width?: string;
  /** 牌背层上移的距离，灰色过渡层取它的一半。 */
  lift?: string;
  className?: string;
}) {
  const style: CSSProperties = {};
  if (width) style["--tile-plate-width" as never] = width as never;
  if (lift) style["--tile-plate-lift" as never] = lift as never;

  return (
    <span
      className={`tile-plate${className ? ` ${className}` : ""}`}
      style={width || lift ? style : undefined}
    >
      <span className="tile-plate__body">
        <span className="tile-plate__face">
          <img src={tileAssetPath(code, "jp")} alt="" />
        </span>
      </span>
    </span>
  );
}
