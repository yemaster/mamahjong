import { useQuery } from "@tanstack/react-query";
import { gameApi, apiFailure } from "../api";
import { Button } from "../components/Button";
import { navigateTo } from "../routing";
import { useAuthStore } from "../stores/authStore";
import type { MatchView } from "../types";

const page: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  alignItems: "center",
  justifyContent: "center",
  height: "100%",
  textAlign: "center",
  padding: 40,
};

const heading: React.CSSProperties = {
  fontSize: 30,
  fontWeight: 900,
  letterSpacing: "0.1em",
  color: "var(--color-gold-bright)",
  textShadow: "0 0 20px var(--color-gold-dim)",
  marginBottom: 36,
};

const placementsRow: React.CSSProperties = {
  display: "flex",
  gap: 24,
  marginBottom: 32,
  flexWrap: "wrap",
  justifyContent: "center",
};

const placementCard = (rank: number): React.CSSProperties => ({
  background: "var(--color-surface)",
  padding: "24px 32px",
  minWidth: 160,
  textAlign: "center",
  border:
    rank === 1
      ? "2px solid var(--color-gold-bright)"
      : "1px solid var(--color-border)",
  boxShadow:
    rank === 1
      ? "0 0 20px var(--color-gold-dim)"
      : "0 2px 8px rgba(0,0,0,0.4)",
});

const rankStyle: React.CSSProperties = {
  fontSize: 36,
  fontWeight: 900,
  color: "var(--color-gold-bright)",
  marginBottom: 8,
};

const nicknameStyle: React.CSSProperties = {
  fontSize: 16,
  fontWeight: 600,
  color: "var(--color-text)",
  marginBottom: 4,
};

const pointsStyle: React.CSSProperties = {
  fontSize: 14,
  color: "var(--color-text-dim)",
};

interface ResultSceneProps {
  matchId: string;
}

export default function ResultScene({ matchId }: ResultSceneProps) {
  const token = useAuthStore((s) => s.token);
  const view = useQuery({
    queryKey: ["matchView", matchId],
    queryFn: () => gameApi.matchView(matchId, token!),
    enabled: !!token,
  });

  if (view.isLoading) {
    return <div style={page}>加载结算…</div>;
  }
  if (view.error) {
    return (
      <div style={{ ...page, color: "var(--color-danger)" }}>
        {apiFailure(view.error).message}
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
      <div style={page}>
        对局未结束
        <div style={{ marginTop: 16 }}>
          <Button onClick={() => navigateTo({ kind: "lobby" })}>
            返回大厅
          </Button>
        </div>
      </div>
    );
  }

  // Sort placements by rank.
  const sorted = [...result.placements].sort((a, b) => a.rank - b.rank);
  const players = new Map(data.players.map((p) => [p.seat, p.nickname]));

  return (
    <div style={page}>
      <h1 style={heading}>对局结束</h1>

      <div style={placementsRow}>
        {sorted.map((p) => (
          <div key={p.seat} style={placementCard(p.rank)}>
            <div style={rankStyle}>
              {p.rank === 1 ? "🥇" : p.rank === 2 ? "🥈" : p.rank === 3 ? "🥉" : "4"}
            </div>
            <div style={nicknameStyle}>
              {players.get(p.seat) ?? `玩家${p.seat}`}
            </div>
            <div style={pointsStyle}>{p.points} 点</div>
          </div>
        ))}
      </div>

      <div style={{ display: "flex", gap: 12 }}>
        <Button
          variant="gold"
          size="lg"
          onClick={() => navigateTo({ kind: "lobby" })}
        >
          返回大厅
        </Button>
      </div>
    </div>
  );
}
