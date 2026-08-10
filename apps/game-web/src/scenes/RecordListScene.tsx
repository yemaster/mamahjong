import { useQuery } from "@tanstack/react-query";
import { apiFailure, gameApi } from "../api";
import { useSceneReady } from "../components/SceneTransition";
import { navigateTo } from "../routing";
import { useAuthStore } from "../stores/authStore";
import {
  formatRecordTime,
  matchRecordTitle,
  orderedSeats,
} from "../replay/recordSummary";
import type { MatchRecordSummary } from "../replay/recordTypes";
import { formatScore } from "./result/resultRows";

const recordsBackground = `${import.meta.env.BASE_URL}assets/ui/sakura-campus-empty.png`;

export default function RecordListScene() {
  const token = useAuthStore((state) => state.token);
  const records = useQuery({
    queryKey: ["records"],
    queryFn: () => gameApi.records(token!),
    enabled: Boolean(token),
  });
  useSceneReady(!records.isLoading);

  return (
    <section className="record-screen" aria-label="牌谱">
      <div
        className="record-screen__background"
        style={{ backgroundImage: `url("${recordsBackground}")` }}
        aria-hidden="true"
      />
      <div className="record-screen__veil" aria-hidden="true" />

      <div className="record-screen__content">
        <header className="record-screen__header">
          <h1>牌谱</h1>
          <button type="button" onClick={() => navigateTo({ kind: "lobby" })}>
            返回大厅
          </button>
        </header>

        <div className="record-screen__list">
          {records.isLoading && <p className="record-screen__hint">加载中…</p>}
          {records.error && (
            <p className="record-screen__hint">
              {apiFailure(records.error).message}
            </p>
          )}
          {records.data?.records.length === 0 && (
            <p className="record-screen__hint">还没有打完的对局。</p>
          )}
          {records.data?.records.map((record) => (
            <RecordCard key={record.match_id} record={record} />
          ))}
        </div>
      </div>
    </section>
  );
}

function RecordCard({ record }: { record: MatchRecordSummary }) {
  return (
    <article className="record-card">
      <header className="record-card__header">
        <h2>{matchRecordTitle(record)}</h2>
        <time className="record-card__time">
          {formatRecordTime(record.finished_at_ms)}
        </time>
        <button
          type="button"
          className="record-card__open"
          onClick={() =>
            navigateTo({ kind: "replay", matchId: record.match_id })
          }
        >
          查看
        </button>
      </header>

      <ol className="record-card__seats">
        {orderedSeats(record).map((seat) => (
          <li key={seat.seat} className="record-card__seat">
            <b className={`record-card__rank is-rank-${seat.rank}`}>
              {seat.rank}
            </b>
            <span className="record-card__nickname">{seat.nickname}</span>
            {/* 增减按马点算：素点减起始点数只是过程，一场的输赢由马点定。 */}
            <span
              className={`record-card__delta ${
                seat.score_tenths >= 0 ? "is-gain" : "is-loss"
              }`}
            >
              {formatScore(seat.score_tenths / 10)}
            </span>
            <span className="record-card__points">{seat.points}</span>
          </li>
        ))}
      </ol>
    </article>
  );
}
