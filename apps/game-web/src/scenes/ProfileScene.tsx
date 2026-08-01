import { useState } from "react";
import { gameApi } from "../api";
import { Button } from "../components/Button";
import { navigateTo } from "../routing";
import { useAuthStore } from "../stores/authStore";

const page: React.CSSProperties = { padding: "36px 44px", height: "100%", overflow: "auto" };
const heading: React.CSSProperties = { fontSize: 22, fontWeight: 800, letterSpacing: "0.08em", color: "var(--pink-dark)", marginBottom: 24 };
const section: React.CSSProperties = { background: "var(--warm-white)", border: "1px solid var(--border)", padding: 20, marginBottom: 14, maxWidth: 460 };
const secLabel: React.CSSProperties = { fontSize: 11, fontWeight: 700, letterSpacing: "0.1em", color: "#7A5C48", textTransform: "uppercase", marginBottom: 6 };
const secValue: React.CSSProperties = { fontSize: 15, fontWeight: 600, letterSpacing: "0.04em", color: "var(--brown)" };
const field: React.CSSProperties = { width: "100%", padding: "10px 14px", marginTop: 6, marginBottom: 10, background: "rgba(0,0,0,0.35)", border: "1px solid var(--border)", borderRadius: "var(--radius-sm)", color: "var(--brown)", fontSize: 14, outline: "none" };

export default function ProfileScene() {
  const { identity, token, setIdentity, logout } = useAuthStore();
  const [editing, setEditing] = useState(false);
  const [nickname, setNickname] = useState(identity?.profile.nickname ?? "");
  const [saving, setSaving] = useState(false);

  return (
    <div style={page}>
      <h2 style={heading}>个人设置</h2>
      {identity && (
        <div style={section}>
          <div style={secLabel}>用户名</div>
          <div style={secValue}>{identity.login_name}</div>
        </div>
      )}
      <div style={section}>
        <div style={secLabel}>昵称</div>
        {editing ? (
          <>
            <input style={field} value={nickname} onChange={(e) => setNickname(e.target.value)} onKeyDown={(e) => e.key === "Enter" && (async () => { if (!token || !nickname) return; setSaving(true); try { setIdentity(await gameApi.updateProfile(token, nickname)); setEditing(false); } finally { setSaving(false); } })()} />
            <div style={{ display: "flex", gap: 8 }}>
              <Button size="sm" onClick={async () => { if (!token || !nickname) return; setSaving(true); try { setIdentity(await gameApi.updateProfile(token, nickname)); setEditing(false); } finally { setSaving(false); } }} disabled={saving}>保存</Button>
              <Button variant="ghost" size="sm" onClick={() => setEditing(false)}>取消</Button>
            </div>
          </>
        ) : (
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
            <div style={secValue}>{identity?.profile.nickname ?? "—"}</div>
            <Button size="sm" onClick={() => setEditing(true)}>编辑</Button>
          </div>
        )}
      </div>
      {identity && identity.profile.ranks.length > 0 && (
        <div style={section}>
          <div style={secLabel}>段位</div>
          {identity.profile.ranks.map((r) => (
            <div key={r.rule_set_id} style={{ marginTop: 4 }}>
              <span style={{ color: "var(--pink-dark)", fontWeight: 700, letterSpacing: "0.06em" }}>{r.rank}</span>
              <span style={{ color: "#7A5C48", fontSize: 12, marginLeft: 8 }}>{r.rule_set_id} · {r.points}pt</span>
            </div>
          ))}
        </div>
      )}
      <div style={{ display: "flex", gap: 10, marginTop: 20 }}>
        <Button variant="pink" onClick={() => navigateTo({ kind: "lobby" })}>返回大厅</Button>
        <Button variant="danger" onClick={() => { logout(); navigateTo({ kind: "lobby" }); }}>退出登录</Button>
      </div>
    </div>
  );
}
