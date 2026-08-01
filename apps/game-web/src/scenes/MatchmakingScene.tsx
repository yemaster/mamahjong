import { useEffect } from "react";
import { useQuery } from "@tanstack/react-query";
import { gameApi, apiFailure } from "../api";
import { Button } from "../components/Button";
import { navigateTo } from "../routing";
import { useAuthStore } from "../stores/authStore";

const page: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  alignItems: "center",
  justifyContent: "center",
  height: "100%",
  textAlign: "center",
};

const spinner: React.CSSProperties = {
  width: 64,
  height: 64,
  border: "4px solid rgba(255,255,255,0.1)",
  borderTop: "4px solid var(--color-accent)",
  borderRadius: "50%",
  animation: "spin 1s linear infinite",
  marginBottom: 24,
};

interface MatchmakingSceneProps {
  ticketId: string;
}

export default function MatchmakingScene({ ticketId }: MatchmakingSceneProps) {
  const token = useAuthStore((s) => s.token);
  const ticket = useQuery({
    queryKey: ["matchmaking", ticketId],
    queryFn: () => gameApi.getTicket(ticketId, token!),
    refetchInterval: 2000,
    enabled: !!token,
  });

  /* Transition to game when matched. */
  useEffect(() => {
    if (ticket.data?.status === "matched" && ticket.data.match_id) {
      navigateTo({ kind: "game", matchId: ticket.data.match_id });
    }
  }, [ticket.data]);

  const cancel = async () => {
    if (!token) return;
    try {
      await gameApi.cancelTicket(ticketId, token);
      navigateTo({ kind: "lobby" });
    } catch {
      /* retry on next poll */
    }
  };

  const retry = () => {
    navigateTo({ kind: "lobby" });
  };

  return (
    <div style={page}>
      <style>{`@keyframes spin { to { transform: rotate(360deg); } }`}</style>
      <div style={spinner} />
      <h2 style={{ fontSize: 20, marginBottom: 8 }}>正在匹配</h2>
      {ticket.error ? (
        <>
          <p
            style={{
              color: "var(--color-danger)",
              fontSize: 14,
              marginBottom: 16,
            }}
          >
            {apiFailure(ticket.error).message}
          </p>
          <Button onClick={retry}>返回大厅</Button>
        </>
      ) : (
        <>
          <p style={{ color: "var(--color-text-dim)", fontSize: 14 }}>
            请稍候，正在寻找对手…
          </p>
          <div style={{ marginTop: 24 }}>
            <Button variant="danger" size="md" onClick={cancel}>
              取消匹配
            </Button>
          </div>
        </>
      )}
    </div>
  );
}
