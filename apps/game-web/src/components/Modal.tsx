import type { ReactNode } from "react";

interface ModalProps {
  open: boolean;
  onClose: () => void;
  title: string;
  children: ReactNode;
}

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
  background: "var(--color-surface)",
  borderRadius: "var(--radius)",
  boxShadow: "var(--shadow-raised)",
  minWidth: 360,
  maxWidth: 480,
  overflow: "hidden",
};

const header: React.CSSProperties = {
  display: "flex",
  justifyContent: "space-between",
  alignItems: "center",
  padding: "16px 20px",
  borderBottom: "1px solid rgba(255,255,255,0.08)",
};

const titleStyle: React.CSSProperties = {
  fontSize: 18,
  fontWeight: 700,
  color: "var(--color-text)",
};

const closeBtn: React.CSSProperties = {
  background: "none",
  border: "none",
  color: "var(--color-text-dim)",
  fontSize: 22,
  cursor: "pointer",
  lineHeight: 1,
};

const body: React.CSSProperties = {
  padding: 20,
};

export function Modal({ open, onClose, title, children }: ModalProps) {
  if (!open) return null;
  return (
    <div style={overlay} onClick={onClose}>
      <div style={card} onClick={(e) => e.stopPropagation()}>
        <div style={header}>
          <span style={titleStyle}>{title}</span>
          <button style={closeBtn} onClick={onClose}>
            ×
          </button>
        </div>
        <div style={body}>{children}</div>
      </div>
    </div>
  );
}
