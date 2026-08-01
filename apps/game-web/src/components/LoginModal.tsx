import { useState } from "react";
import { gameApi } from "../api";
import { useAuthStore } from "../stores/authStore";
import { Button } from "./Button";
import { Modal } from "./Modal";

/* ══════ Game-style input ══════ */

const gameInput: React.CSSProperties = {
  width: "100%",
  padding: "10px 14px",
  marginBottom: 14,
  background: "rgba(0,0,0,0.35)",
  border: "1px solid var(--color-border)",
  borderRadius: "var(--radius-sm)",
  color: "var(--color-text)",
  fontSize: 14,
  outline: "none",
  transition: "border-color 0.2s",
  letterSpacing: "0.04em",
};

const gameInputFocus: React.CSSProperties = {
  borderColor: "var(--color-gold-bright)",
  boxShadow: "0 0 6px var(--color-gold-dim)",
};

const errorStyle: React.CSSProperties = {
  color: "#e88",
  fontSize: 12,
  marginBottom: 10,
  letterSpacing: "0.04em",
};

/* ── Tab ──────────────────────────────── */

const tabBar: React.CSSProperties = {
  display: "flex",
  marginBottom: 18,
};

const tabBase: React.CSSProperties = {
  flex: 1,
  padding: "8px 0",
  background: "transparent",
  color: "var(--color-text-dim)",
  border: "none",
  borderBottom: "2px solid transparent",
  fontSize: 14,
  fontWeight: 600,
  letterSpacing: "0.06em",
  cursor: "pointer",
  transition: "all 0.15s",
};

const tabActive: React.CSSProperties = {
  ...tabBase,
  color: "var(--color-gold-bright)",
  borderBottomColor: "var(--color-gold-bright)",
};

/* ══════ Component ══════ */

interface LoginModalProps {
  open: boolean;
  onClose: () => void;
}

export function LoginModal({ open, onClose }: LoginModalProps) {
  const { setToken, setIdentity } = useAuthStore();
  const [tab, setTab] = useState<"login" | "register">("login");
  const [loginName, setLoginName] = useState("");
  const [password, setPassword] = useState("");
  const [nickname, setNickname] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const submit = async () => {
    setError(null);
    setLoading(true);
    try {
      const response =
        tab === "login"
          ? await gameApi.login(loginName, password)
          : await gameApi.register(loginName, password, nickname);
      setToken(response.session.token);
      const identity = await gameApi.me(response.session.token);
      setIdentity(identity);
      onClose();
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : "操作失败，请重试");
    } finally {
      setLoading(false);
    }
  };

  return (
    <Modal open={open} onClose={onClose} title="登录">
      <div style={tabBar}>
        <button
          style={tab === "login" ? tabActive : tabBase}
          onClick={() => setTab("login")}
        >
          登录
        </button>
        <button
          style={tab === "register" ? tabActive : tabBase}
          onClick={() => setTab("register")}
        >
          注册
        </button>
      </div>
      {error && <div style={errorStyle}>{error}</div>}
      <input
        style={gameInput}
        placeholder="用户名"
        value={loginName}
        onChange={(e) => setLoginName(e.target.value)}
        onKeyDown={(e) => e.key === "Enter" && submit()}
        onFocus={(e) =>
          Object.assign((e.target as HTMLElement).style, gameInputFocus)
        }
        onBlur={(e) =>
          Object.assign((e.target as HTMLElement).style, gameInput)
        }
      />
      <input
        style={gameInput}
        type="password"
        placeholder="密码"
        value={password}
        onChange={(e) => setPassword(e.target.value)}
        onKeyDown={(e) => e.key === "Enter" && submit()}
        onFocus={(e) =>
          Object.assign((e.target as HTMLElement).style, gameInputFocus)
        }
        onBlur={(e) =>
          Object.assign((e.target as HTMLElement).style, gameInput)
        }
      />
      {tab === "register" && (
        <input
          style={gameInput}
          placeholder="昵称"
          value={nickname}
          onChange={(e) => setNickname(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && submit()}
          onFocus={(e) =>
            Object.assign((e.target as HTMLElement).style, gameInputFocus)
          }
          onBlur={(e) =>
            Object.assign((e.target as HTMLElement).style, gameInput)
          }
        />
      )}
      <Button
        variant="gold"
        size="md"
        onClick={submit}
        disabled={loading || !loginName || !password}
        style={{ width: "100%", marginTop: 6 }}
      >
        {loading ? "请稍候…" : tab === "login" ? "登录" : "注册"}
      </Button>
    </Modal>
  );
}
