import type { ReactNode } from "react";

interface ModalProps {
  open: boolean;
  onClose: () => void;
  title: string;
  children: ReactNode;
}

/* ══════ Styles ══════ */

const overlay: React.CSSProperties = {
  position: "fixed",
  inset: 0,
  background: "var(--color-overlay)",
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  zIndex: 1000,
};

const card: React.CSSProperties = {
  background: "#0f1f18",
  border: "1px solid var(--color-border)",
  boxShadow:
    "0 0 30px rgba(0,0,0,0.7), 0 0 60px rgba(140,110,30,0.08), inset 0 1px 0 rgba(255,255,255,0.03)",
  minWidth: 360,
  maxWidth: 480,
};

/* ── Header ──────────────────────── */

const header: React.CSSProperties = {
  display: "flex",
  justifyContent: "space-between",
  alignItems: "center",
  padding: "14px 20px",
  borderBottom: "1px solid var(--color-border)",
  background: "rgba(0,0,0,0.2)",
};

const titleStyle: React.CSSProperties = {
  fontSize: 17,
  fontWeight: 700,
  letterSpacing: "0.08em",
  color: "var(--color-gold-bright)",
};

const closeBtn: React.CSSProperties = {
  background: "none",
  border: "1px solid transparent",
  color: "var(--color-text-dim)",
  fontSize: 20,
  width: 32,
  height: 32,
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  cursor: "pointer",
  transition: "all 0.15s",
  borderRadius: "var(--radius-sm)",
};

const closeHover: React.CSSProperties = {
  borderColor: "var(--color-border)",
  color: "var(--color-text)",
};

const body: React.CSSProperties = {
  padding: 20,
};

/* ══════ Component ══════ */

export function Modal({ open, onClose, title, children }: ModalProps) {
  if (!open) return null;
  return (
    <div style={overlay} onClick={onClose}>
      <div style={card} onClick={(e) => e.stopPropagation()}>
        <div style={header}>
          <span style={titleStyle}>{title}</span>
          <button
            style={closeBtn}
            onClick={onClose}
            onMouseEnter={(e) =>
              Object.assign((e.target as HTMLElement).style, closeHover)
            }
            onMouseLeave={(e) =>
              Object.assign((e.target as HTMLElement).style, closeBtn)
            }
          >
            ✕
          </button>
        </div>
        <div style={body}>{children}</div>
      </div>
    </div>
  );
}
