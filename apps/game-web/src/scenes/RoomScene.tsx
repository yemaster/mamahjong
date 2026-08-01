import { useQuery } from "@tanstack/react-query";
import { gameApi, apiFailure } from "../api";
import { Button } from "../components/Button";
import { navigateTo } from "../routing";
import { useAuthStore } from "../stores/authStore";

const page: React.CSSProperties = {
  padding: "32px 40px",
  height: "100%",
  overflow: "auto",
};

const heading: React.CSSProperties = {
  fontSize: 22,
  fontWeight: 700,
  marginBottom: 24,
};

const section: React.CSSProperties = {
  background: "var(--color-surface)",
  borderRadius: "var(--radius)",
  padding: 20,
  marginBottom: 16,
  maxWidth: 560,
};

const memberRow: React.CSSProperties = {
  display: "flex",
  justifyContent: "space-between",
  alignItems: "center",
  padding: "8px 0",
  borderBottom: "1px solid rgba(255,255,255,0.04)",
};

const statusDot: Record<string, React.CSSProperties> = {
  ready: {
    width: 8,
    height: 8,
    borderRadius: "50%",
    background: "var(--color-success)",
    display: "inline-block",
    marginRight: 8,
  },
  waiting: {
    width: 8,
    height: 8,
    borderRadius: "50%",
    background: "var(--color-accent)",
    display: "inline-block",
    marginRight: 8,
  },
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

  if (room.isLoading) {
    return <div style={page}>加载中…</div>;
  }
  if (room.error) {
    return (
      <div style={{ ...page, color: "var(--color-danger)" }}>
        {apiFailure(room.error).message}
        <br />
        <Button
          variant="ghost"
          size="sm"
          onClick={() => navigateTo({ kind: "lobby" })}
          style={{ marginTop: 12 }}
        >
          返回大厅
        </Button>
      </div>
    );
  }
  if (!room.data) return null;

  const data = room.data;
  const isOwner = data.owner_user_id === identity?.id;
  const isMember = data.members.some((m) => m.user_id === identity?.id);
  const allReady =
    data.members.length >= 2 && data.members.every((m) => m.ready);

  /* Transition to game when match starts. */
  if (data.active_match_id) {
    navigateTo({ kind: "game", matchId: data.active_match_id });
    return null;
  }

  const doJoin = async () => {
    if (!token) return;
    try {
      await gameApi.joinRoom(roomId, data.version, token);
      room.refetch();
    } catch {
      /* error shown by refresh */
    }
  };

  const doLeave = async () => {
    if (!token) return;
    try {
      await gameApi.leaveRoom(roomId, data.version, token);
      navigateTo({ kind: "lobby" });
    } catch {
      /* error shown by refresh */
    }
  };

  const doReady = async (ready: boolean) => {
    if (!token) return;
    try {
      await gameApi.setReady(roomId, data.version, ready, token);
      room.refetch();
    } catch {
      /* error shown by refresh */
    }
  };

  const doStart = async () => {
    if (!token) return;
    try {
      await gameApi.startRoom(roomId, data.version, token);
      room.refetch();
    } catch {
      /* error shown by refresh */
    }
  };

  return (
    <div style={page}>
      <h2 style={heading}>{data.name}</h2>

      <div style={section}>
        <h3 style={{ fontSize: 15, marginBottom: 12 }}>成员</h3>
        {data.members.map((member) => (
          <div key={member.user_id} style={memberRow}>
            <span style={{ display: "flex", alignItems: "center" }}>
              <span
                style={
                  member.ready ? statusDot.ready : statusDot.waiting
                }
              />
              {member.nickname}
              {member.user_id === data.owner_user_id && " · 房主"}
            </span>
            <span style={{ fontSize: 13, color: "var(--color-text-dim)" }}>
              {member.ready ? "已准备" : "未准备"}
            </span>
          </div>
        ))}
        {data.members.length < 4 && (
          <div style={{ marginTop: 12, color: "var(--color-text-dim)" }}>
            等待更多玩家加入…
          </div>
        )}
      </div>

      <div style={{ display: "flex", gap: 12, flexWrap: "wrap" }}>
        {!isMember && (
          <Button variant="primary" onClick={doJoin}>
            加入房间
          </Button>
        )}
        {isMember && (
          <Button variant="primary" onClick={() => doReady(true)}>
            {data.members.find((m) => m.user_id === identity?.id)?.ready
              ? "取消准备"
              : "准备"}
          </Button>
        )}
        {isOwner && (
          <Button
            variant="primary"
            onClick={doStart}
            disabled={!allReady}
          >
            开始对局
          </Button>
        )}
        <Button variant="ghost" onClick={doLeave}>
          离开
        </Button>
        <Button
          variant="ghost"
          size="sm"
          onClick={() => navigateTo({ kind: "lobby" })}
        >
          返回大厅
        </Button>
      </div>
    </div>
  );
}
