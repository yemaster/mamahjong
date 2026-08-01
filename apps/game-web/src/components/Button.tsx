import type { ButtonHTMLAttributes, ReactNode } from "react";

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: "gold" | "danger" | "ghost";
  size?: "sm" | "md" | "lg";
  glow?: boolean;
  children: ReactNode;
}

/* ══════ Base ══════ */

const base: React.CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  justifyContent: "center",
  gap: 6,
  fontWeight: 700,
  letterSpacing: "0.06em",
  textTransform: "uppercase",
  cursor: "pointer",
  transition: "all 0.12s ease",
  position: "relative",
  userSelect: "none",
};

/* ══════ Variants ══════ */

const goldStyle: React.CSSProperties = {
  background: "linear-gradient(180deg, #2a2010 0%, #1a1408 100%)",
  color: "var(--color-gold-bright)",
  border: "1px solid var(--color-gold-dim)",
  boxShadow: "inset 0 1px 0 rgba(255,255,255,0.06), 0 2px 4px rgba(0,0,0,0.5)",
};

const goldHover: React.CSSProperties = {
  borderColor: "var(--color-gold-bright)",
  boxShadow:
    "inset 0 1px 0 rgba(255,255,255,0.08), 0 0 10px var(--color-gold-dim), 0 2px 4px rgba(0,0,0,0.5)",
  color: "#fff",
};

const goldActive: React.CSSProperties = {
  transform: "scale(0.97)",
  boxShadow: "inset 0 2px 4px rgba(0,0,0,0.4), 0 0 6px var(--color-gold-dim)",
};

const dangerStyle: React.CSSProperties = {
  background: "linear-gradient(180deg, #3a1814 0%, #24100c 100%)",
  color: "#e88",
  border: "1px solid #733",
  boxShadow: "inset 0 1px 0 rgba(255,255,255,0.04), 0 2px 4px rgba(0,0,0,0.5)",
};

const dangerHover: React.CSSProperties = {
  borderColor: "#c44",
  boxShadow: "inset 0 1px 0 rgba(255,255,255,0.06), 0 0 10px var(--color-danger-glow)",
  color: "#faa",
};

const dangerActive: React.CSSProperties = {
  transform: "scale(0.97)",
  boxShadow: "inset 0 2px 4px rgba(0,0,0,0.4), 0 0 6px var(--color-danger-glow)",
};

const ghostStyle: React.CSSProperties = {
  background: "transparent",
  color: "var(--color-text-dim)",
  border: "1px solid transparent",
};

const ghostHover: React.CSSProperties = {
  color: "var(--color-text)",
  borderColor: "var(--color-border)",
};

/* ══════ Sizes ══════ */

const sizes: Record<string, React.CSSProperties> = {
  sm: { padding: "5px 14px", fontSize: 12, borderRadius: "var(--radius-sm)" },
  md: { padding: "9px 22px", fontSize: 14, borderRadius: "var(--radius-sm)" },
  lg: { padding: "13px 30px", fontSize: 16, borderRadius: "var(--radius)" },
};

/* ══════ Component ══════ */

const variantStyles: Record<
  string,
  { base: React.CSSProperties; hover: React.CSSProperties; active: React.CSSProperties }
> = {
  gold:   { base: goldStyle,   hover: goldHover,   active: goldActive },
  danger: { base: dangerStyle, hover: dangerHover, active: dangerActive },
  ghost:  { base: ghostStyle,  hover: ghostHover,  active: { transform: "scale(0.98)" } },
};

export function Button({
  variant = "gold",
  size = "md",
  glow = false,
  style,
  disabled,
  children,
  ...props
}: ButtonProps) {
  const v = variantStyles[variant] ?? variantStyles.gold!;
  const merged: React.CSSProperties = {
    ...base,
    ...v.base,
    ...sizes[size],
    ...(glow ? { animation: "glowPulse 2.5s ease-in-out infinite" } : {}),
    ...(disabled
      ? { opacity: 0.35, cursor: "not-allowed", pointerEvents: "none" }
      : {}),
    ...style,
  };

  return (
    <button
      style={merged}
      disabled={disabled}
      onMouseEnter={(e) => {
        if (disabled) return;
        Object.assign((e.target as HTMLElement).style, v.hover);
      }}
      onMouseLeave={(e) => {
        if (disabled) return;
        Object.assign((e.target as HTMLElement).style, v.base);
      }}
      onMouseDown={(e) => {
        if (disabled) return;
        Object.assign((e.target as HTMLElement).style, v.active);
      }}
      onMouseUp={(e) => {
        if (disabled) return;
        Object.assign((e.target as HTMLElement).style, v.hover);
      }}
      {...props}
    >
      {children}
    </button>
  );
}
