import { useQuery } from "@tanstack/react-query";
import { gameApi, apiFailure } from "../api";
import { Button } from "../components/Button";
import { navigateTo } from "../routing";
import { useAuthStore } from "../stores/authStore";

/* ══════ Styles ══════ */

const page: React.CSSProperties = {
  padding: "36px 44px",
  height: "100%",
  overflow: "auto",
};

const heading: React.CSSProperties = {
  fontSize: 22,
  fontWeight: 800,
  letterSpacing: "0.08em",
  color: "var(--pink-dark)",
  marginBottom: 24,
};

const section: React.CSSProperties = {
  background: "var(--warm-white)",
  border: "1px solid var(--border)",
  padding: 20,
  marginBottom: 16,
  maxWidth: 520,
};

const sectionTitle: React.CSSProperties = {
  fontSize: 13,
  fontWeight: 700,
  letterSpacing: "0.08em",
  color: "#7A5C48",
  textTransform: "uppercase",
  marginBottom: 14,
  paddingBottom: 8,
  borderBottom: "1px solid var(--border)",
};

const memberRow: React.CSSProperties = {
  display: "flex",
  justifyContent: "space-between",
  alignItems: "center",
  padding: "8px 0",
  borderBottom: "1px solid rgba(255,255,255,0.04)",
  fontSize: 14,
};

const readyDot = (ready: boolean): React.CSSProperties => ({
  width: 8,
  height: 8,
  borderRadius: "50%",
  background: ready ? "var(--mint)" : "var(--pink)",
  boxShadow: ready
    ? "0 0 6px var(--mint)"
    : "0 0 6px var(--pink)",
  display: "inline-block",
  marginRight: 10,
});

const statusText: React.CSSProperties = {
  fontSize: 12,
  letterSpacing: "0.06em",
};

interface RoomSceneProps {
  roomId: string;
}

export default function RoomScene({ roomId }: RoomSceneProps) {
  const token = useAuthStore((s) => s.token);
  const identity = useAuthStore((s) => s.identity);
  const room = useQuery({
    queryKey: ["room", roomId],
    queryFn: () => gameApi.getRoom(roomId),
    refetchInterval: 2000,
  });

  if (room.isLoading)
    return <div style={{ padding: 40, color: "#7A5C48" }}>加载中…</div>;
  if (room.error)
    return (
      <div style={{ padding: 40, color: "var(--red-soft)" }}>
        {apiFailure(room.error).message}
        <div style={{ marginTop: 12 }}>
          <Button variant="ghost" size="sm" onClick={() => navigateTo({ kind: "lobby" })}>
            返回大厅
          </Button>
        </div>
      </div>
    );
  if (!room.data) return null;

  const data = room.data;
  const isOwner = data.owner_user_id === identity?.id;
  const isMember = data.members.some((m) => m.user_id === identity?.id);
  const selfMember = data.members.find((m) => m.user_id === identity?.id);
  const allReady =
    data.members.length >= 2 && data.members.every((m) => m.ready);

  if (data.active_match_id) {
    navigateTo({ kind: "game", matchId: data.active_match_id });
    return null;
  }

  const call = (fn: () => Promise<unknown>) => async () => {
    try {
      await fn();
      room.refetch();
    } catch { /* shown on next poll */ }
  };

  return (
    <div style={page}>
      <h2 style={heading}>{data.name}</h2>

      <div style={section}>
        <div style={sectionTitle}>成员</div>
        {data.members.map((m) => (
          <div key={m.user_id} style={memberRow}>
            <span style={{ display: "flex", alignItems: "center" }}>
              <span style={readyDot(m.ready)} />
              {m.nickname}
              {m.user_id === data.owner_user_id && (
                <span style={{ color: "var(--pink)", marginLeft: 8, fontSize: 11 }}>
                  房主
                </span>
              )}
            </span>
            <span style={statusText}>
              {m.ready ? "已准备" : "等待中"}
            </span>
          </div>
        ))}
      </div>

      <div style={{ display: "flex", gap: 10, flexWrap: "wrap" }}>
        {!isMember && (
          <Button variant="pink" onClick={call(() => gameApi.joinRoom(roomId, data.version, token!))}>
            加入房间
          </Button>
        )}
        {isMember && (
          <Button
            variant="pink"
            onClick={call(() => gameApi.setReady(roomId, data.version, !selfMember?.ready, token!))}
          >
            {selfMember?.ready ? "取消准备" : "准备"}
          </Button>
        )}
        {isOwner && (
          <Button
            variant="pink"
            onClick={call(() => gameApi.startRoom(roomId, data.version, token!))}
            disabled={!allReady}
          >
            开始对局
          </Button>
        )}
        <Button variant="ghost" size="sm" onClick={call(() => gameApi.leaveRoom(roomId, data.version, token!).then(() => navigateTo({ kind: "lobby" })))}>
          离开
        </Button>
        <Button variant="ghost" size="sm" onClick={() => navigateTo({ kind: "lobby" })}>
          返回大厅
        </Button>
      </div>
    </div>
  );
}
