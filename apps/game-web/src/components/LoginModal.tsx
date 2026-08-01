import { useState } from "react";
import { gameApi } from "../api";
import { useAuthStore } from "../stores/authStore";
import { Button } from "./Button";
import { Modal } from "./Modal";

const fieldStyle: React.CSSProperties = {
  width: "100%",
  padding: "10px 14px",
  marginBottom: 12,
  background: "var(--color-surface-raised)",
  border: "1px solid rgba(255,255,255,0.1)",
  borderRadius: "var(--radius-sm)",
  color: "var(--color-text)",
  fontSize: 15,
  outline: "none",
};

const errorStyle: React.CSSProperties = {
  color: "var(--color-danger)",
  fontSize: 13,
  marginBottom: 8,
};

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
      setError(
        err instanceof Error ? err.message : "操作失败，请重试",
      );
    } finally {
      setLoading(false);
    }
  };

  return (
    <Modal open={open} onClose={onClose} title="登录">
      <div style={{ display: "flex", gap: 0, marginBottom: 16 }}>
        <TabButton
          active={tab === "login"}
          onClick={() => setTab("login")}
        >
          登录
        </TabButton>
        <TabButton
          active={tab === "register"}
          onClick={() => setTab("register")}
        >
          注册
        </TabButton>
      </div>
      {error && <div style={errorStyle}>{error}</div>}
      <input
        style={fieldStyle}
        placeholder="用户名"
        value={loginName}
        onChange={(e) => setLoginName(e.target.value)}
        onKeyDown={(e) => e.key === "Enter" && submit()}
      />
      <input
        style={fieldStyle}
        type="password"
        placeholder="密码"
        value={password}
        onChange={(e) => setPassword(e.target.value)}
        onKeyDown={(e) => e.key === "Enter" && submit()}
      />
      {tab === "register" && (
        <input
          style={fieldStyle}
          placeholder="昵称"
          value={nickname}
          onChange={(e) => setNickname(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && submit()}
        />
      )}
      <Button
        variant="primary"
        size="md"
        onClick={submit}
        disabled={loading || !loginName || !password}
        style={{ width: "100%", marginTop: 4 }}
      >
        {loading ? "请稍候…" : tab === "login" ? "登录" : "注册"}
      </Button>
    </Modal>
  );
}

function TabButton({
  active,
  onClick,
  children,
}: {
  active: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      onClick={onClick}
      style={{
        flex: 1,
        padding: "8px 0",
        background: active ? "var(--color-surface-raised)" : "transparent",
        color: active ? "var(--color-accent)" : "var(--color-text-dim)",
        border: "none",
        borderBottom: active
          ? "2px solid var(--color-accent)"
          : "2px solid transparent",
        fontSize: 15,
        cursor: "pointer",
      }}
    >
      {children}
    </button>
  );
}
