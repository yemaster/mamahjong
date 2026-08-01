import type { ButtonHTMLAttributes, ReactNode } from "react";

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: "gold" | "danger" | "ghost";
  size?: "sm" | "md" | "lg";
  children: ReactNode;
}

const base: React.CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  justifyContent: "center",
  gap: 6,
  fontWeight: 700,
  letterSpacing: "0.06em",
  cursor: "pointer",
  transition: "background 0.12s, border-color 0.12s, color 0.12s, transform 0.08s",
  position: "relative",
  userSelect: "none",
};

const gold: React.CSSProperties = {
  ...base,
  background: "#1a1408",
  color: "#e8c547",
  border: "1px solid #8a6d28",
};

const goldHover: React.CSSProperties = {
  background: "#2a2010",
  borderColor: "#c9a034",
  color: "#f5eed6",
};

const goldActive: React.CSSProperties = {
  transform: "scale(0.97)",
};

const danger: React.CSSProperties = {
  ...base,
  background: "#24100c",
  color: "#e88",
  border: "1px solid #733",
};

const dangerHover: React.CSSProperties = {
  background: "#3a1814",
  borderColor: "#c44",
};

const dangerActive: React.CSSProperties = {
  transform: "scale(0.97)",
};

const ghost: React.CSSProperties = {
  ...base,
  background: "transparent",
  color: "#7a7668",
  border: "1px solid transparent",
};

const ghostHover: React.CSSProperties = {
  color: "#f0e6c8",
  borderColor: "rgba(180,140,60,0.25)",
};

const sizes: Record<string, React.CSSProperties> = {
  sm: { padding: "5px 14px", fontSize: 12, borderRadius: 2 },
  md: { padding: "9px 22px", fontSize: 14, borderRadius: 2 },
  lg: { padding: "13px 30px", fontSize: 16, borderRadius: 4 },
};

const styles = { gold, goldHover, goldActive, danger, dangerHover, dangerActive, ghost, ghostHover };

export function Button({
  variant = "gold",
  size = "md",
  style,
  disabled,
  children,
  ...props
}: ButtonProps) {
  const v = styles[variant] ?? styles.gold;
  const h = styles[`${variant}Hover` as keyof typeof styles] as React.CSSProperties | undefined;
  const a = styles[`${variant}Active` as keyof typeof styles] as React.CSSProperties | undefined;

  const merged: React.CSSProperties = {
    ...v,
    ...sizes[size],
    ...(disabled
      ? { opacity: 0.3, cursor: "not-allowed", pointerEvents: "none" }
      : {}),
    ...style,
  };

  return (
    <button
      style={merged}
      disabled={disabled}
      onMouseEnter={(e) => {
        if (disabled || !h) return;
        Object.assign((e.target as HTMLElement).style, h);
      }}
      onMouseLeave={(e) => {
        if (disabled) return;
        Object.assign((e.target as HTMLElement).style, v);
      }}
      onMouseDown={(e) => {
        if (disabled || !a) return;
        Object.assign((e.target as HTMLElement).style, a);
      }}
      onMouseUp={(e) => {
        if (disabled || !h) return;
        Object.assign((e.target as HTMLElement).style, h);
      }}
      {...props}
    >
      {children}
    </button>
  );
}
