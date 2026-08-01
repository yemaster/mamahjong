import type { ReactNode } from "react";
import { useGameStore } from "../stores/gameStore";

/* ══════ Game HUD top bar ══════ */

const topBar: React.CSSProperties = {
  display: "flex",
  justifyContent: "space-between",
  alignItems: "center",
  padding: "0 20px",
  height: 40,
  background: "var(--color-surface)",
  borderBottom: "1px solid var(--color-gold-dim)",
  flexShrink: 0,
};

const logoArea: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 10,
};

const logoText: React.CSSProperties = {
  fontSize: 16,
  fontWeight: 800,
  letterSpacing: "0.12em",
  color: "var(--color-gold-bright)",
  textShadow: "0 1px 3px rgba(0,0,0,0.5)",
};

const right: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 14,
  fontSize: 12,
  color: "var(--color-text-dim)",
  letterSpacing: "0.05em",
};

const dotBase: React.CSSProperties = {
  width: 8,
  height: 8,
  borderRadius: "50%",
  display: "inline-block",
};

/* ══════ Component ══════ */

interface LayoutProps {
  children: ReactNode;
}

export function Layout({ children }: LayoutProps) {
  const wsState = useGameStore((s) => s.wsState);

  const dotStyle: React.CSSProperties =
    wsState === "connected"
      ? {
          ...dotBase,
          background: "var(--color-success)",
          boxShadow: "0 0 6px var(--color-success)",
        }
      : wsState === "connecting"
        ? {
            ...dotBase,
            background: "var(--color-gold-bright)",
            boxShadow: "0 0 6px var(--color-gold-dim)",
            animation: "glowPulse 1.5s ease-in-out infinite",
          }
        : {
            ...dotBase,
            background: "var(--color-offline)",
          };

  return (
    <div
      style={{ display: "flex", flexDirection: "column", height: "100%" }}
    >
      <div style={topBar}>
        <div style={logoArea}>
          <span style={logoText}>麻麻的将</span>
        </div>
        <div style={right}>
          <span
            style={{
              display: "flex",
              alignItems: "center",
              gap: 6,
            }}
          >
            <span style={dotStyle} />
            {wsState !== "connected" && "离线"}
          </span>
        </div>
      </div>
      <main style={{ flex: 1, overflow: "hidden", position: "relative" }}>
        {children}
      </main>
    </div>
  );
}
