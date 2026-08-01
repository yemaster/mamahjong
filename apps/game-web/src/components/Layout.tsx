import type { ReactNode } from "react";
import { useGameStore } from "../stores/gameStore";

const topBar: React.CSSProperties = {
  display: "flex",
  justifyContent: "space-between",
  alignItems: "center",
  padding: "8px 20px",
  height: 48,
  background: "var(--color-surface)",
  borderBottom: "1px solid rgba(255,255,255,0.06)",
  flexShrink: 0,
};

const logo: React.CSSProperties = {
  fontSize: 18,
  fontWeight: 700,
  color: "var(--color-accent)",
  userSelect: "none",
};

const right: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 16,
  fontSize: 13,
};

const dot: React.CSSProperties = {
  width: 8,
  height: 8,
  borderRadius: "50%",
  display: "inline-block",
};

interface LayoutProps {
  children: ReactNode;
}

export function Layout({ children }: LayoutProps) {
  const wsState = useGameStore((s) => s.wsState);

  const dotColor =
    wsState === "connected"
      ? "var(--color-success)"
      : wsState === "connecting"
        ? "var(--color-accent)"
        : "var(--color-offline)";

  return (
    <div
      style={{ display: "flex", flexDirection: "column", height: "100%" }}
    >
      <div style={topBar}>
        <span style={logo}>麻麻的将</span>
        <div style={right}>
          <span style={{ display: "flex", alignItems: "center", gap: 4 }}>
            <span
              style={{ ...dot, background: dotColor }}
              title={
                wsState === "connected"
                  ? "已连接"
                  : wsState === "connecting"
                    ? "连接中"
                    : "离线"
              }
            />
            {wsState !== "connected" && "离线"}
          </span>
        </div>
      </div>
      <main
        style={{ flex: 1, overflow: "hidden", position: "relative" }}
      >
        {children}
      </main>
    </div>
  );
}
