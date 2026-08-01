import { useState } from "react";
import { gameApi } from "../api";
import { Button } from "../components/Button";
import { navigateTo } from "../routing";
import { useAuthStore } from "../stores/authStore";

const page: React.CSSProperties = {
  padding: "32px 40px",
  height: "100%",
  overflow: "auto",
};

const heading: React.CSSProperties = {
  fontSize: 22,
  fontWeight: 700,
  marginBottom: 24,
};

const section: React.CSSProperties = {
  background: "var(--color-surface)",
  borderRadius: "var(--radius)",
  padding: 20,
  marginBottom: 16,
  maxWidth: 480,
};

const label: React.CSSProperties = {
  fontSize: 13,
  color: "var(--color-text-dim)",
  marginBottom: 4,
};

const value: React.CSSProperties = {
  fontSize: 16,
  color: "var(--color-text)",
};

const field: React.CSSProperties = {
  width: "100%",
  padding: "10px 14px",
  marginTop: 8,
  marginBottom: 12,
  background: "var(--color-surface-raised)",
  border: "1px solid rgba(255,255,255,0.1)",
  borderRadius: "var(--radius-sm)",
  color: "var(--color-text)",
  fontSize: 15,
  outline: "none",
};

export default function ProfileScene() {
  const { identity, token, setIdentity, logout } = useAuthStore();
  const [editing, setEditing] = useState(false);
  const [nickname, setNickname] = useState(
    identity?.profile.nickname ?? "",
  );
  const [saving, setSaving] = useState(false);

  const saveNickname = async () => {
    if (!token || !nickname) return;
    setSaving(true);
    try {
      const updated = await gameApi.updateProfile(token, nickname);
      setIdentity(updated);
      setEditing(false);
    } catch {
      /* keep editing */
    } finally {
      setSaving(false);
    }
  };

  return (
    <div style={page}>
      <h2 style={heading}>个人设置</h2>

      {identity && (
        <div style={section}>
          <div style={label}>用户名</div>
          <div style={value}>{identity.login_name}</div>
        </div>
      )}

      <div style={section}>
        <div style={label}>昵称</div>
        {editing ? (
          <>
            <input
              style={field}
              value={nickname}
              onChange={(e) => setNickname(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && saveNickname()}
            />
            <div style={{ display: "flex", gap: 8 }}>
              <Button size="sm" onClick={saveNickname} disabled={saving}>
                保存
              </Button>
              <Button
                variant="ghost"
                size="sm"
                onClick={() => setEditing(false)}
              >
                取消
              </Button>
            </div>
          </>
        ) : (
          <div style={{ display: "flex", justifyContent: "space-between" }}>
            <div style={value}>
              {identity?.profile.nickname ?? "—"}
            </div>
            <Button size="sm" onClick={() => setEditing(true)}>
              编辑
            </Button>
          </div>
        )}
      </div>

      {identity && (
        <div style={section}>
          <div style={label}>角色</div>
          <div style={{ ...value, color: "var(--color-text-dim)" }}>
            {identity.profile.selected_character?.name ??
              "未选择（占位）"}
          </div>
        </div>
      )}

      {identity && identity.profile.ranks.length > 0 && (
        <div style={section}>
          <div style={label}>段位</div>
          {identity.profile.ranks.map((rank) => (
            <div key={rank.rule_set_id} style={{ marginTop: 4 }}>
              <span style={{ color: "var(--color-accent)", fontWeight: 600 }}>
                {rank.rank}
              </span>
              <span style={{ color: "var(--color-text-dim)", fontSize: 13 }}>
                {" "}
                · {rank.rule_set_id} · {rank.points}pt
              </span>
            </div>
          ))}
        </div>
      )}

      <div style={{ display: "flex", gap: 12, marginTop: 24 }}>
        <Button onClick={() => navigateTo({ kind: "lobby" })}>
          返回大厅
        </Button>
        <Button
          variant="danger"
          onClick={() => {
            logout();
            navigateTo({ kind: "lobby" });
          }}
        >
          退出登录
        </Button>
      </div>
    </div>
  );
}
