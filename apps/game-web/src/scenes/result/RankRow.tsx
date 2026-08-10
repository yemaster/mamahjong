import type { CSSProperties } from "react";
import {
  deltaTone,
  formatDelta,
  formatScore,
  type ResultRow,
} from "./resultRows";

const fallbackAvatar =
  `${import.meta.env.BASE_URL}assets/local-characters/mahjong-soul/ichihime/emotes/8.png`;

interface RankRowProps {
  row: ResultRow;
  /** 这一条等多久才飞进来。 */
  enterDelayMs: number;
}

/**
 * 一条名次条：菱形名次牌骑在左边缘上，往右依次是头像、昵称、点数、末列。
 *
 * 两列数字各占一列固定宽度，四条才能对齐成竖列。走金色那套配色的是**自家**那
 * 条：谁是一位菱形牌上写着，四条里找自己却只能挨个读昵称。
 *
 * 末列摆什么看规则：冲击麻将有杠点这本单独的账，末列写杠点增减，点数那列底下
 * 再补一行点数增减；立直麻将没有杠点，末列照旧是马点。
 */
export function RankRow({ row, enterDelayMs }: RankRowProps) {
  const tone =
    row.score > 0 ? " is-plus" : row.score < 0 ? " is-minus" : " is-even";

  return (
    <div
      className={`result-rank-row${row.isSelf ? " is-self" : ""}`}
      style={{ "--enter-delay": `${enterDelayMs}ms` } as CSSProperties}
    >
      <span className="result-rank-row__badge" aria-hidden="true">
        <b>{row.rank}位</b>
      </span>
      <img
        className="result-rank-row__avatar"
        src={row.avatarPath ?? fallbackAvatar}
        alt=""
        onError={(event) => {
          (event.target as HTMLImageElement).src = fallbackAvatar;
        }}
      />
      <span className="result-rank-row__name">{row.nickname}</span>
      <span
        className={`result-rank-row__points${
          row.pointDelta != null ? " is-stacked" : ""
        }`}
      >
        {row.points}
        {row.pointDelta != null && (
          <i
            className={`result-rank-row__delta${deltaTone(row.pointDelta)}`}
            title="点数增减"
          >
            {formatDelta(row.pointDelta)}
          </i>
        )}
      </span>
      {row.kanPointDelta != null ? (
        <span
          className={`result-rank-row__score${deltaTone(row.kanPointDelta)}`}
        >
          <i className="result-rank-row__caption">杠点</i>
          {formatDelta(row.kanPointDelta)}
        </span>
      ) : (
        <span className={`result-rank-row__score${tone}`}>
          {formatScore(row.score)}
        </span>
      )}
    </div>
  );
}
