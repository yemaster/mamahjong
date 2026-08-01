import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { gameApi, apiFailure } from "../api";
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

const fieldStyle: React.CSSProperties = {
  width: "100%",
  maxWidth: 400,
  padding: "10px 14px",
  marginBottom: 16,
  background: "var(--color-surface-raised)",
  border: "1px solid rgba(255,255,255,0.1)",
  borderRadius: "var(--radius-sm)",
  color: "var(--color-text)",
  fontSize: 15,
  outline: "none",
};

const selectStyle: React.CSSProperties = {
  ...fieldStyle,
  cursor: "pointer",
  appearance: "auto",
};

const labelStyle: React.CSSProperties = {
  fontSize: 13,
  color: "var(--color-text-dim)",
  marginBottom: 4,
};

export default function CreateRoomScene() {
  const token = useAuthStore((s) => s.token);
  const catalog = useQuery({
    queryKey: ["ruleSets"],
    queryFn: () => gameApi.ruleSets(),
  });

  const [name, setName] = useState("新房间");
  const [ruleSetId, setRuleSetId] = useState("riichi/yonma");
  const [presetId, setPresetId] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const selectedRuleSet = catalog.data?.rule_sets.find(
    (rs) => rs.id === ruleSetId,
  );
  const presets = selectedRuleSet?.presets ?? [];

  const submit = async () => {
    if (!token) return;
    setError(null);
    setLoading(true);
    try {
      const config: Record<string, unknown> = {};
      if (presetId) {
        config.preset = { id: presetId, revision: 1 };
      }
      const room = await gameApi.createRoom(
        {
          name,
          visibility: "public",
          rules: {
            rule_set_id: ruleSetId,
            config,
          },
        },
        token,
      );
      navigateTo({ kind: "room", roomId: room.id });
    } catch (err: unknown) {
      setError(apiFailure(err).message);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div style={page}>
      <h2 style={heading}>创建房间</h2>
      {catalog.error && (
        <div style={{ color: "var(--color-danger)", marginBottom: 16 }}>
          无法加载规则：{apiFailure(catalog.error).message}
        </div>
      )}
      {error && (
        <div style={{ color: "var(--color-danger)", marginBottom: 16 }}>
          {error}
        </div>
      )}
      <div style={labelStyle}>房间名称</div>
      <input
        style={fieldStyle}
        value={name}
        onChange={(e) => setName(e.target.value)}
        onKeyDown={(e) => e.key === "Enter" && submit()}
      />
      <div style={labelStyle}>规则</div>
      <select
        style={selectStyle}
        value={ruleSetId}
        onChange={(e) => {
          setRuleSetId(e.target.value);
          setPresetId("");
        }}
      >
        {catalog.data?.rule_sets.map((rs) => (
          <option key={rs.id} value={rs.id}>
            {rs.name}（{rs.seat_count}人）
          </option>
        )) ?? (
          <option value="riichi/yonma">四人麻将</option>
        )}
      </select>
      {presets.length > 0 && (
        <>
          <div style={labelStyle}>预设（可选）</div>
          <select
            style={selectStyle}
            value={presetId}
            onChange={(e) => setPresetId(e.target.value)}
          >
            <option value="">默认规则</option>
            {presets.map((p) => (
              <option key={p.id} value={p.id}>
                {p.display_name}
              </option>
            ))}
          </select>
        </>
      )}
      <div style={{ display: "flex", gap: 12, marginTop: 8 }}>
        <Button
          variant="primary"
          size="lg"
          onClick={submit}
          disabled={loading || !name}
        >
          创建
        </Button>
        <Button
          variant="ghost"
          size="lg"
          onClick={() => navigateTo({ kind: "lobby" })}
        >
          取消
        </Button>
      </div>
    </div>
  );
}
