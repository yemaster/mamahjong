import { useQuery } from "@tanstack/react-query";
import { gameApi, apiFailure } from "../api";
import { Button } from "../components/Button";
import { navigateTo } from "../routing";
import { useAuthStore } from "../stores/authStore";
import type { RoomView } from "../types";

const page: React.CSSProperties = {
  padding: "32px 40px",
  height: "100%",
  overflow: "auto",
};

const title: React.CSSProperties = {
  fontSize: 22,
  fontWeight: 700,
  marginBottom: 24,
};

const grid: React.CSSProperties = {
  display: "grid",
  gridTemplateColumns: "repeat(auto-fill, minmax(300px, 1fr))",
  gap: 16,
};

const cardStyle: React.CSSProperties = {
  background: "var(--color-surface)",
  borderRadius: "var(--radius)",
  padding: 20,
  border: "1px solid rgba(255,255,255,0.06)",
};

const cardTitle: React.CSSProperties = {
  fontSize: 16,
  fontWeight: 600,
  marginBottom: 8,
};

const cardMeta: React.CSSProperties = {
  fontSize: 13,
  color: "var(--color-text-dim)",
  marginBottom: 12,
};

const actions: React.CSSProperties = {
  display: "flex",
  gap: 12,
  marginTop: 24,
  flexWrap: "wrap",
};

const empty: React.CSSProperties = {
  color: "var(--color-text-dim)",
  fontSize: 15,
  textAlign: "center",
  paddingTop: 80,
};

export default function LobbyScene() {
  const token = useAuthStore((s) => s.token);
  const rooms = useQuery({
    queryKey: ["rooms"],
    queryFn: () => gameApi.rooms(),
    refetchInterval: 5000,
  });

  const joinRoom = (room: RoomView) => {
    navigateTo({ kind: "room", roomId: room.id });
  };

  return (
    <div style={page}>
      <h2 style={title}>大厅</h2>
      <div style={actions}>
        <Button
          variant="primary"
          size="lg"
          onClick={() => navigateTo({ kind: "create-room" })}
        >
          创建房间
        </Button>
        <Button
          size="lg"
          onClick={() => {
            if (token) {
              gameApi
                .enterMatchmaking("riichi/yonma", token)
                .then((ticket) =>
                  navigateTo({ kind: "matchmaking", ticketId: ticket.id }),
                )
                .catch(() => {});
            }
          }}
        >
          四人匹配
        </Button>
        <Button
          size="lg"
          onClick={() => {
            if (token) {
              gameApi
                .enterMatchmaking("riichi/sanma", token)
                .then((ticket) =>
                  navigateTo({ kind: "matchmaking", ticketId: ticket.id }),
                )
                .catch(() => {});
            }
          }}
        >
          三人匹配
        </Button>
        <Button
          variant="ghost"
          size="sm"
          onClick={() => navigateTo({ kind: "profile" })}
        >
          个人设置
        </Button>
      </div>

      {rooms.isLoading && (
        <div style={{ color: "var(--color-text-dim)", marginTop: 40 }}>
          加载中…
        </div>
      )}
      {rooms.error && (
        <div style={{ color: "var(--color-danger)", marginTop: 40 }}>
          {apiFailure(rooms.error).message}
        </div>
      )}
      {rooms.data?.rooms.length === 0 && (
        <div style={empty}>暂无公开房间，创建一个吧！</div>
      )}
      {rooms.data && rooms.data.rooms.length > 0 && (
        <>
          <h3 style={{ marginTop: 32, marginBottom: 16, fontSize: 16 }}>
            公开房间
          </h3>
          <div style={grid}>
            {rooms.data.rooms.map((room) => (
              <button
                key={room.id}
                style={{
                  ...cardStyle,
                  cursor: "pointer",
                  textAlign: "left",
                  border: "1px solid rgba(255,255,255,0.06)",
                  background: "var(--color-surface)",
                }}
                onClick={() => joinRoom(room)}
              >
                <div style={cardTitle}>{room.name}</div>
                <div style={cardMeta}>
                  {room.members.length}人 ·{" "}
                  {room.lifecycle === "waiting" ? "等待中" : "对局中"} ·{" "}
                  {room.visibility === "public" ? "公开" : "私有"}
                </div>
              </button>
            ))}
          </div>
        </>
      )}
    </div>
  );
}
