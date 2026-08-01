import { useGameStore } from "../stores/gameStore";

const bar: React.CSSProperties = {
  position: "absolute",
  bottom: 0,
  left: 0,
  right: 0,
  display: "flex",
  justifyContent: "center",
  gap: 16,
  padding: "8px 16px",
  background: "rgba(0,0,0,0.5)",
  fontSize: 14,
  fontFamily: "var(--font-game)",
  zIndex: 10,
};

const seatChip: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 8,
  padding: "4px 12px",
  borderRadius: "var(--radius-sm)",
  background: "var(--warm-white)",
};

export function ClockBar() {
  const clocks = useGameStore((s) => s.clocks);
  const view = useGameStore((s) => s.matchView);

  if (!view || clocks.size === 0) return null;

  const entries = Array.from(clocks.values());
  const playerNames = new Map(
    view.players.map((p) => [p.seat, p.nickname]),
  );

  return (
    <div style={bar}>
      {entries.map((c) => {
        const totalS = Math.ceil(c.remaining_ms / 1000);
        const pct =
          c.base_ms + c.reserve_ms > 0
            ? c.remaining_ms / (c.base_ms + c.reserve_ms)
            : 0;
        const color =
          totalS <= 5 ? "var(--red-soft)" : totalS <= 10 ? "var(--pink)" : "var(--mint)";
        return (
          <div key={c.seat} style={seatChip}>
            <span style={{ color: "#7A5C48" }}>
              {playerNames.get(c.seat) ?? `玩家${c.seat}`}
            </span>
            <span style={{ color, fontWeight: 700, fontVariantNumeric: "tabular-nums" }}>
              {c.base_ms === 0 ? `长考 ${totalS}s` : `${totalS}s`}
            </span>
          </div>
        );
      })}
    </div>
  );
}
