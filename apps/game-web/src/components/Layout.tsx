import type { ReactNode } from "react";
import { useGameStore } from "../stores/gameStore";

const bar: React.CSSProperties = {
  display: "flex", justifyContent: "space-between", alignItems: "center",
  padding: "0 20px", height: 42,
  background: "var(--warm-white)", borderBottom: "1px solid var(--border)", flexShrink: 0,
};
const logo: React.CSSProperties = {
  fontSize: 17, fontWeight: 800, letterSpacing: "0.1em", color: "var(--pink-dark)",
};
const right: React.CSSProperties = {
  display: "flex", alignItems: "center", gap: 10, fontSize: 11, color: "#7A5C48",
};
const dotBase: React.CSSProperties = { width: 7, height: 7, borderRadius: "50%", display: "inline-block" };

interface Props { children: ReactNode; }

export function Layout({ children }: Props) {
  const s = useGameStore((st) => st.wsState);
  const d: React.CSSProperties =
    s === "connected" ? { ...dotBase, background: "var(--mint)" } :
    s === "connecting" ? { ...dotBase, background: "var(--pink)", animation: "breathe 1.5s ease-in-out infinite" } :
    { ...dotBase, background: "var(--red-soft)" };

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100%" }}>
      <div style={bar}>
        <span style={logo}>麻麻的将</span>
        <div style={right}><span style={{ display: "flex", alignItems: "center", gap: 6 }}><span style={d} />{s !== "connected" && "离线"}</span></div>
      </div>
      <main style={{ flex: 1, overflow: "hidden", position: "relative" }}>{children}</main>
    </div>
  );
}
