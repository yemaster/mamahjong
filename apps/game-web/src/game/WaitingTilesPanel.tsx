import type { WaitingTileView } from "../types";
import { tileRemaining } from "./tileCounts";
import { TilePlate } from "./TilePlate";

/** 听牌框只排一行，放不下的听牌收到末尾的省略号里。 */
export const MAX_WAIT_TILES = 8;

interface WaitingTilesPanelProps {
  waitingTiles: WaitingTileView[];
  /** 立直选牌和摸切预览的那块面板要压在别的界面之上。 */
  preview?: boolean;
  /**
   * 当前视角已经看见的牌数，用来算每张听牌还剩几枚。
   * 不给就只排牌、不带任何标签——流局摊牌走的就是这一种。
   */
  visibleCounts?: Map<string, number>;
  className?: string;
}

/**
 * 自己的听牌提示和流局摊牌共用的听牌框：一行实体牌，牌型和主视角手牌完全一致，
 * 只是整体小一号。
 */
export function WaitingTilesPanel({
  waitingTiles,
  preview = false,
  visibleCounts,
  className = "",
}: WaitingTilesPanelProps) {
  const shown = waitingTiles.slice(0, MAX_WAIT_TILES);
  const hidden = waitingTiles.length - shown.length;

  return (
    <aside
      className={`match-wait-assist__panel${
        preview ? " match-wait-assist__panel--riichi" : ""
      }${className ? ` ${className}` : ""}`}
      aria-label="听牌一览"
    >
      {shown.map((waitingTile) => (
        <span className="match-wait-assist__wait" key={waitingTile.code}>
          <TilePlate code={waitingTile.code} />
          {visibleCounts &&
            (waitingTile.has_yaku ? (
              /* 有役时关心的是这张还剩几枚能和，没役才要提醒和不了。 */
              <b className="has-yaku">
                {tileRemaining(visibleCounts, waitingTile.code)}枚
              </b>
            ) : (
              <b className="no-yaku">无役</b>
            ))}
        </span>
      ))}
      {hidden > 0 && (
        <span
          className="match-wait-assist__more"
          aria-label={`另有 ${hidden} 种听牌`}
        >
          …
        </span>
      )}
    </aside>
  );
}
