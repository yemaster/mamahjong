import type { ReactNode } from "react";

interface ModalProps { open: boolean; onClose: () => void; title: string; children: ReactNode; }

const ov: React.CSSProperties = { position:"fixed", inset:0, background:"var(--overlay)", display:"flex", alignItems:"center", justifyContent:"center", zIndex:1000 };
const ca: React.CSSProperties = { background:"var(--warm-white)", border:"1px solid var(--border)", boxShadow:"0 4px 20px rgba(74,55,40,0.12)", minWidth:360, maxWidth:480 };
const hd: React.CSSProperties = { display:"flex", justifyContent:"space-between", alignItems:"center", padding:"14px 20px", borderBottom:"1px solid var(--border)", background:"rgba(244,167,185,0.06)" };
const ti: React.CSSProperties = { fontSize:17, fontWeight:700, letterSpacing:"0.06em", color:"var(--pink-dark)" };
const cx: React.CSSProperties = { background:"none", border:"1px solid transparent", color:"#7A5C48", fontSize:20, width:32, height:32, display:"flex", alignItems:"center", justifyContent:"center", cursor:"pointer", borderRadius:4 };
const cxH: React.CSSProperties = { borderColor:"var(--border)", color:"var(--brown)" };
const bd: React.CSSProperties = { padding:20 };

export function Modal({ open, onClose, title, children }: ModalProps) {
  if (!open) return null;
  return <div style={ov} onClick={onClose}><div style={ca} onClick={e => e.stopPropagation()}>
    <div style={hd}><span style={ti}>{title}</span><button style={cx} onClick={onClose} onMouseEnter={e => Object.assign((e.target as HTMLElement).style, cxH)} onMouseLeave={e => Object.assign((e.target as HTMLElement).style, cx)}>✕</button></div>
    <div style={bd}>{children}</div>
  </div></div>;
}
