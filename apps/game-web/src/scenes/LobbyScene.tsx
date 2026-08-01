import { useQuery } from "@tanstack/react-query";
import { gameApi, apiFailure } from "../api";
import { Button } from "../components/Button";
import { navigateTo } from "../routing";
import { useAuthStore } from "../stores/authStore";
import type { RoomView } from "../types";

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
  color: "var(--color-gold-bright)",
  marginBottom: 28,
};

const grid: React.CSSProperties = {
  display: "grid",
  gridTemplateColumns: "repeat(auto-fill, minmax(290px, 1fr))",
  gap: 14,
};

const cardBase: React.CSSProperties = {
  background: "var(--color-surface)",
  border: "1px solid var(--color-border)",
  padding: 18,
  cursor: "pointer",
  textAlign: "left",
  transition: "border-color 0.15s, box-shadow 0.15s",
};

const cardHover: React.CSSProperties = {
  borderColor: "var(--color-border-glow)",
  boxShadow: "0 0 12px rgba(180,140,60,0.1)",
};

const cardName: React.CSSProperties = {
  fontSize: 16,
  fontWeight: 700,
  letterSpacing: "0.05em",
  color: "var(--color-gold-bright)",
  marginBottom: 8,
};

const cardMeta: React.CSSProperties = {
  fontSize: 12,
  letterSpacing: "0.04em",
  color: "var(--color-text-dim)",
};

const actions: React.CSSProperties = {
  display: "flex",
  gap: 14,
  marginTop: 30,
  marginBottom: 28,
  flexWrap: "wrap",
};

const sectionTitle: React.CSSProperties = {
  fontSize: 13,
  fontWeight: 700,
  letterSpacing: "0.1em",
  color: "var(--color-text-dim)",
  textTransform: "uppercase",
  marginBottom: 14,
  paddingBottom: 8,
  borderBottom: "1px solid var(--color-border)",
};

const empty: React.CSSProperties = {
  color: "var(--color-text-dim)",
  fontSize: 14,
  letterSpacing: "0.04em",
  textAlign: "center",
  paddingTop: 80,
};

const errorStyle: React.CSSProperties = {
  color: "#e88",
  fontSize: 13,
  marginTop: 16,
};

/* ══════ Component ══════ */

export default function LobbyScene() {
  const token = useAuthStore((s) => s.token);
  const rooms = useQuery({
    queryKey: ["rooms"],
    queryFn: () => gameApi.rooms(),
    refetchInterval: 5000,
  });

  const quickMatch = (variant: string) => {
    if (!token) return;
    gameApi
      .enterMatchmaking(`riichi/${variant}`, token)
      .then((ticket) =>
        navigateTo({ kind: "matchmaking", ticketId: ticket.id }),
      )
      .catch(() => {});
  };

  return (
    <div style={page}>
      <h2 style={heading}>大厅</h2>

      <div style={actions}>
        <Button
          variant="gold"
          size="lg"
          onClick={() => navigateTo({ kind: "create-room" })}
        >
          创建房间
        </Button>
        <Button size="lg" onClick={() => quickMatch("yonma")}>
          四人匹配
        </Button>
        <Button size="lg" onClick={() => quickMatch("sanma")}>
          三人匹配
        </Button>
      </div>

      {rooms.isLoading && (
        <div style={{ color: "var(--color-text-dim)", marginTop: 40 }}>
          加载中…
        </div>
      )}
      {rooms.error && (
        <div style={errorStyle}>{apiFailure(rooms.error).message}</div>
      )}
      {rooms.data?.rooms.length === 0 && (
        <div style={empty}>暂无公开房间</div>
      )}
      {rooms.data && rooms.data.rooms.length > 0 && (
        <>
          <div style={sectionTitle}>公开房间</div>
          <div style={grid}>
            {rooms.data.rooms.map((room) => (
              <RoomCard key={room.id} room={room} />
            ))}
          </div>
        </>
      )}
    </div>
  );
}

function RoomCard({ room }: { room: RoomView }) {
  return (
    <button
      style={cardBase}
      onClick={() => navigateTo({ kind: "room", roomId: room.id })}
      onMouseEnter={(e) =>
        Object.assign((e.target as HTMLElement).style, cardHover)
      }
      onMouseLeave={(e) =>
        Object.assign((e.target as HTMLElement).style, cardBase)
      }
    >
      <div style={cardName}>{room.name}</div>
      <div style={cardMeta}>
        {room.members.length} 人 ·{" "}
        {room.lifecycle === "waiting" ? "等待中" : "对局中"} ·{" "}
        {room.visibility === "public" ? "公开" : "私有"}
      </div>
    </button>
  );
}
