import type { ButtonHTMLAttributes, ReactNode } from "react";

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: "pink" | "danger" | "ghost";
  size?: "sm" | "md" | "lg";
  children: ReactNode;
}

const b: React.CSSProperties = {
  display: "inline-flex", alignItems: "center", justifyContent: "center", gap: 6,
  fontWeight: 700, letterSpacing: "0.06em", cursor: "pointer",
  transition: "background 0.12s, border-color 0.12s, color 0.12s, transform 0.08s",
  userSelect: "none",
};

const pink: React.CSSProperties  = { ...b, background: "#FDE4EC", color: "#D4899E", border: "1px solid #F4A7B9" };
const pinkH: React.CSSProperties  = { background: "#F4A7B9", color: "#fff", borderColor: "#D4899E" };
const pinkA: React.CSSProperties  = { transform: "scale(0.97)" };

const danger: React.CSSProperties = { ...b, background: "#FDE8E8", color: "#E88B8B", border: "1px solid #F0C0C0" };
const dangerH: React.CSSProperties = { background: "#E88B8B", color: "#fff", borderColor: "#D07070" };
const dangerA: React.CSSProperties = { transform: "scale(0.97)" };

const ghost: React.CSSProperties  = { ...b, background: "transparent", color: "#7A5C48", border: "1px solid transparent" };
const ghostH: React.CSSProperties  = { color: "#4A3728", borderColor: "rgba(180,140,100,0.3)" };

const sz: Record<string, React.CSSProperties> = {
  sm: { padding: "5px 14px", fontSize: 12, borderRadius: 4 },
  md: { padding: "9px 22px", fontSize: 14, borderRadius: 4 },
  lg: { padding: "13px 30px", fontSize: 16, borderRadius: 6 },
};

const all = { pink, pinkH, pinkA, danger, dangerH, dangerA, ghost, ghostH };

export function Button({ variant = "pink", size = "md", style, disabled, children, ...props }: ButtonProps) {
  const v = (all as Record<string, React.CSSProperties>)[variant] ?? all.pink;
  const h = (all as Record<string, React.CSSProperties>)[`${variant}H`];
  const a = (all as Record<string, React.CSSProperties>)[`${variant}A`];
  const s = sz[size] ?? sz.md;

  return (
    <button
      style={{ ...v, ...s, ...(disabled ? { opacity: 0.35, cursor: "not-allowed", pointerEvents: "none" } : {}), ...style }}
      disabled={disabled}
      onMouseEnter={(e) => { if (disabled || !h) return; Object.assign((e.target as HTMLElement).style, h); }}
      onMouseLeave={(e) => { if (disabled) return; Object.assign((e.target as HTMLElement).style, { ...v, ...s }); }}
      onMouseDown={(e) => { if (disabled || !a) return; Object.assign((e.target as HTMLElement).style, a); }}
      onMouseUp={(e) => { if (disabled || !h) return; Object.assign((e.target as HTMLElement).style, h); }}
      {...props}
    >
      {children}
    </button>
  );
}
