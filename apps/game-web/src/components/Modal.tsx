import { type ReactNode, useEffect, useState } from "react";

interface ModalProps { open: boolean; onClose: () => void; title: string; children: ReactNode; dismissible?: boolean; }

const MODAL_TRANSITION_DURATION = 220;

export function Modal({ open, onClose, title, children, dismissible = true }: ModalProps) {
  const [mounted, setMounted] = useState(open);
  const [visible, setVisible] = useState(false);

  useEffect(() => {
    if (open) {
      setMounted(true);
      const frame = requestAnimationFrame(() => setVisible(true));
      return () => cancelAnimationFrame(frame);
    }

    setVisible(false);
    const timer = setTimeout(
      () => setMounted(false),
      MODAL_TRANSITION_DURATION,
    );
    return () => clearTimeout(timer);
  }, [open]);

  if (!mounted) return null;
  return <div className={`modal-overlay${visible ? " is-visible" : ""}`} onClick={dismissible ? onClose : undefined}><div className="modal-card" onClick={e => e.stopPropagation()}>
    <div className="modal-header"><span className="modal-title">{title}</span>{dismissible && <button className="modal-close" onClick={onClose}>✕</button>}</div>
    <div className="modal-body">{children}</div>
  </div></div>;
}
