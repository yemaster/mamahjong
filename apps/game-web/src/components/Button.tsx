import type { ButtonHTMLAttributes, ReactNode } from "react";

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: "primary" | "danger" | "ghost";
  size?: "sm" | "md" | "lg";
  children: ReactNode;
}

const base: React.CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  justifyContent: "center",
  gap: 6,
  border: "1px solid transparent",
  borderRadius: "var(--radius-sm)",
  fontFamily: "var(--font-game)",
  fontWeight: 600,
  cursor: "pointer",
  transition: "filter 0.15s",
  whiteSpace: "nowrap",
};

const variants: Record<string, React.CSSProperties> = {
  primary: {
    background: "var(--color-accent)",
    color: "#1A1A2E",
    borderColor: "var(--color-accent)",
  },
  danger: {
    background: "var(--color-danger)",
    color: "#fff",
    borderColor: "var(--color-danger)",
  },
  ghost: {
    background: "transparent",
    color: "var(--color-text)",
    borderColor: "var(--color-text-dim)",
  },
};

const sizes: Record<string, React.CSSProperties> = {
  sm: { padding: "4px 12px", fontSize: 13 },
  md: { padding: "8px 20px", fontSize: 15 },
  lg: { padding: "12px 28px", fontSize: 17 },
};

export function Button({
  variant = "primary",
  size = "md",
  style,
  disabled,
  children,
  ...props
}: ButtonProps) {
  const merged: React.CSSProperties = {
    ...base,
    ...variants[variant],
    ...sizes[size],
    ...(disabled ? { opacity: 0.4, cursor: "not-allowed" } : {}),
    ...style,
  };
  return (
    <button style={merged} disabled={disabled} {...props}>
      {children}
    </button>
  );
}
