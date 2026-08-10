import { useEffect, type CSSProperties } from "react";
import { useQuery } from "@tanstack/react-query";
import { ApiError, gameApi, apiFailure } from "../api";
import { Button } from "../components/Button";
import { useSceneReady } from "../components/SceneTransition";
import { navigateTo } from "../routing";
import { useAuthStore } from "../stores/authStore";
import type { MatchView } from "../types";
import { RankRow } from "./result/RankRow";
import { resultRows, rowEnterDelayMs, rowsSettledMs } from "./result/resultRows";

const resultBackground =
  `${import.meta.env.BASE_URL}assets/ui/sakura-campus-empty.png`;

interface ResultSceneProps {
  matchId: string;
}

export default function ResultScene({ matchId }: ResultSceneProps) {
  const token = useAuthStore((s) => s.token);
  const view = useQuery({
    queryKey: ["matchView", matchId],
    queryFn: () => gameApi.matchView(matchId, token!),
    enabled: !!token,
    retry: (failureCount, error) =>
      !(error instanceof ApiError && error.status === 404) &&
      failureCount < 2,
  });
  const matchMissing =
    view.error instanceof ApiError && view.error.status === 404;

  useEffect(() => {
    if (matchMissing) {
      navigateTo({ kind: "lobby" });
    }
  }, [matchMissing]);

  useSceneReady(!view.isLoading);

  if (view.isLoading) {
    return (
      <div className="result-loading">
        <span className="result-loading__mark">終</span>
        <p>加载结算…</p>
      </div>
    );
  }
  if (view.error) {
    return (
      <div className="result-error">
        {matchMissing
          ? "对局不存在，正在返回大厅…"
          : apiFailure(view.error).message}
        <div style={{ marginTop: 16 }}>
          <Button onClick={() => navigateTo({ kind: "lobby" })}>
            返回大厅
          </Button>
        </div>
      </div>
    );
  }

  const data = view.data as MatchView;
  const result = data.result;
  if (!result) {
    return (
      <div className="result-error">
        对局未结束
        <div style={{ marginTop: 16 }}>
          <Button onClick={() => navigateTo({ kind: "lobby" })}>
            返回大厅
          </Button>
        </div>
      </div>
    );
  }

  const rows = resultRows(data, result);
  const champion = data.players.find((player) => player.seat === rows[0]?.seat);

  return (
    <div className="result-screen">
      {/* 大厅那张校园背景压成夜色，终局和大厅是同一个地方，天黑了而已。 */}
      <div
        className="result-screen__scenery"
        style={{ backgroundImage: `url(${resultBackground})` }}
      />
      <div className="result-screen__night" />

      {/* 左侧：冠军立绘，底端出血，右缘化进夜色 */}
      {champion?.character_illustration_path && (
        <div className="result-screen__hero">
          <img src={champion.character_illustration_path} alt="" />
        </div>
      )}

      {/* 右侧：名次条 */}
      <div className="result-screen__rankings">
        <div className="result-screen__rows">
          {rows.map((row) => (
            <RankRow
              key={row.seat}
              row={row}
              enterDelayMs={rowEnterDelayMs(row.rank, rows.length)}
            />
          ))}
        </div>
        <div
          className="result-screen__actions"
          style={
            { "--enter-delay": `${rowsSettledMs(rows.length)}ms` } as CSSProperties
          }
        >
          <button
            className="result-screen__confirm"
            type="button"
            onClick={() => navigateTo({ kind: "room", roomId: data.room_id })}
          >
            返回房间
          </button>
        </div>
      </div>
    </div>
  );
}
