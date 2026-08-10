import { TilePlate } from "../game/TilePlate";
import type { TileView } from "../types";
import type { WallSnapshot } from "./recordTypes";

/**
 * 牌山面板。
 *
 * 按洗好的顺序把一局的牌山摊出来，四张一组，分三段：开局发牌吃掉的那一段、
 * 之后对局中一张张摸走的那一段、最后十四张王牌。三段之间画一条分割线。
 *
 * 颜色三档：当前视角自己摸到的打黄，已经被别人摸走的打灰，还没摸到的留白。
 * 谁摸了哪张是从事件日志里的牌 id 反查出来的（见 `replayState.ts` 的 `drawnBy`）。
 * 三档颜色不写图例——牌面本身就是图例。
 */

const GROUP_SIZE = 4;
/** 王牌固定十四张。 */
const DEAD_WALL_SIZE = 14;

export interface WallPanelProps {
  wall: WallSnapshot | null | undefined;
  /** 每一张已经被摸走的牌是谁摸的。 */
  drawnBy: Map<number, number>;
  observerSeat: number;
  seatCount: number;
  onClose: () => void;
}

function WallSection({
  title,
  tiles,
  drawnBy,
  observerSeat,
}: {
  title: string;
  tiles: TileView[];
  drawnBy: Map<number, number>;
  observerSeat: number;
}) {
  const groups: TileView[][] = [];
  for (let index = 0; index < tiles.length; index += GROUP_SIZE) {
    groups.push(tiles.slice(index, index + GROUP_SIZE));
  }
  return (
    <section className="replay-wall__section">
      <h3>
        {title}
        <span>{tiles.length}张</span>
      </h3>
      <div className="replay-wall__groups">
        {groups.map((group, groupIndex) => (
          <div className="replay-wall__group" key={groupIndex}>
            {group.map((tile) => {
              const taker = drawnBy.get(tile.id);
              const state =
                taker === observerSeat
                  ? "is-mine"
                  : taker !== undefined
                    ? "is-taken"
                    : "is-fresh";
              return (
                <TilePlate
                  key={tile.id}
                  code={tile.code}
                  className={`replay-wall__tile ${state}`}
                />
              );
            })}
          </div>
        ))}
      </div>
    </section>
  );
}

export function WallPanel({
  wall,
  drawnBy,
  observerSeat,
  seatCount,
  onClose,
}: WallPanelProps) {
  const tiles = wall?.tiles ?? [];
  /* 开局每家十三张，摸完这一段才轮到庄家第一次摸牌。 */
  const dealtCount = Math.min(tiles.length, seatCount * 13);
  const liveEnd = Math.min(
    wall?.live_end ?? tiles.length,
    Math.max(dealtCount, tiles.length - DEAD_WALL_SIZE),
  );

  return (
    <aside className="replay-wall" aria-label="牌山">
      <header className="replay-wall__header">
        <h2>牌山</h2>
        <button type="button" onClick={onClose} aria-label="关闭牌山">
          ✕
        </button>
      </header>

      {tiles.length === 0 ? (
        /* 牌山是后来才写进牌谱的，早先归档的那些局没有这一段。 */
        <p className="replay-wall__empty">该对局无牌山记录</p>
      ) : (
        <div className="replay-wall__body">
          <WallSection
            title="开局配牌"
            tiles={tiles.slice(0, dealtCount)}
            drawnBy={drawnBy}
            observerSeat={observerSeat}
          />
          <WallSection
            title="对局摸牌"
            tiles={tiles.slice(dealtCount, liveEnd)}
            drawnBy={drawnBy}
            observerSeat={observerSeat}
          />
          <WallSection
            title="王牌"
            tiles={tiles.slice(liveEnd)}
            drawnBy={drawnBy}
            observerSeat={observerSeat}
          />
        </div>
      )}
    </aside>
  );
}
