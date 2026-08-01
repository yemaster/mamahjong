import { useState } from "react";
import { gameApi } from "../api";
import { useAuthStore } from "../stores/authStore";
import { Button } from "./Button";
import { Modal } from "./Modal";

const fi: React.CSSProperties = { width:"100%", padding:"10px 14px", marginBottom:14, background:"#fff", border:"1px solid var(--border)", borderRadius:"var(--radius)", color:"var(--brown)", fontSize:14, outline:"none", letterSpacing:"0.04em" };
const fiF: React.CSSProperties = { borderColor:"var(--pink)" };
const er: React.CSSProperties = { color:"var(--red-soft)", fontSize:12, marginBottom:10 };
const tb: React.CSSProperties = { display:"flex", marginBottom:18 };
const ta: React.CSSProperties = { flex:1, padding:"8px 0", background:"transparent", color:"#7A5C48", border:"none", borderBottom:"2px solid transparent", fontSize:14, fontWeight:600, letterSpacing:"0.06em", cursor:"pointer" };
const ti: React.CSSProperties = { ...ta, color:"var(--pink-dark)", borderBottomColor:"var(--pink)" };

interface Props { open: boolean; onClose: () => void; }

export function LoginModal({ open, onClose }: Props) {
  const { setToken, setIdentity } = useAuthStore();
  const [tab, setTab] = useState<"login"|"register">("login");
  const [ln, setLn] = useState(""); const [pw, setPw] = useState(""); const [nn, setNn] = useState("");
  const [error, setError] = useState<string|null>(null); const [ld, setLd] = useState(false);

  const sub = async () => {
    setError(null); setLd(true);
    try {
      const r = tab==="login" ? await gameApi.login(ln,pw) : await gameApi.register(ln,pw,nn);
      setToken(r.session.token); setIdentity(await gameApi.me(r.session.token)); onClose();
    } catch(e: unknown) { setError(e instanceof Error ? e.message : "操作失败"); }
    finally { setLd(false); }
  };

  return <Modal open={open} onClose={onClose} title="登录">
    <div style={tb}>
      <button style={tab==="login"?ti:ta} onClick={()=>setTab("login")}>登录</button>
      <button style={tab==="register"?ti:ta} onClick={()=>setTab("register")}>注册</button>
    </div>
    {error && <div style={er}>{error}</div>}
    <input style={fi} placeholder="用户名" value={ln} onChange={e=>setLn(e.target.value)} onKeyDown={e=>e.key==="Enter"&&sub()} onFocus={e=>Object.assign((e.target as HTMLElement).style,fiF)} onBlur={e=>Object.assign((e.target as HTMLElement).style,fi)} />
    <input style={fi} type="password" placeholder="密码" value={pw} onChange={e=>setPw(e.target.value)} onKeyDown={e=>e.key==="Enter"&&sub()} onFocus={e=>Object.assign((e.target as HTMLElement).style,fiF)} onBlur={e=>Object.assign((e.target as HTMLElement).style,fi)} />
    {tab==="register" && <input style={fi} placeholder="昵称" value={nn} onChange={e=>setNn(e.target.value)} onKeyDown={e=>e.key==="Enter"&&sub()} onFocus={e=>Object.assign((e.target as HTMLElement).style,fiF)} onBlur={e=>Object.assign((e.target as HTMLElement).style,fi)} />}
    <Button variant="pink" size="md" onClick={sub} disabled={ld||!ln||!pw} style={{width:"100%",marginTop:6}}>{ld?"请稍候…":tab==="login"?"登录":"注册"}</Button>
  </Modal>;
}
