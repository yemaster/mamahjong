import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { gameApi, apiFailure } from "../api";
import { Button } from "../components/Button";
import { navigateTo } from "../routing";
import { useAuthStore } from "../stores/authStore";

const page: React.CSSProperties = {
  padding: "36px 44px",
  height: "100%",
  overflow: "auto",
};
const heading: React.CSSProperties = {
  fontSize: 22,
  fontWeight: 800,
  letterSpacing: "0.08em",
  color: "var(--pink-dark)",
  marginBottom: 24,
};
const label: React.CSSProperties = {
  fontSize: 12,
  fontWeight: 700,
  letterSpacing: "0.08em",
  color: "#7A5C48",
  textTransform: "uppercase",
  marginBottom: 4,
};
const field: React.CSSProperties = {
  width: "100%",
  maxWidth: 400,
  padding: "10px 14px",
  marginBottom: 16,
  background: "rgba(0,0,0,0.35)",
  border: "1px solid var(--border)",
  borderRadius: "var(--radius-sm)",
  color: "var(--brown)",
  fontSize: 14,
  outline: "none",
  letterSpacing: "0.04em",
};

export default function CreateRoomScene() {
  const token = useAuthStore((s) => s.token);
  const catalog = useQuery({
    queryKey: ["ruleSets"],
    queryFn: () => gameApi.ruleSets(),
  });
  const [name, setName] = useState("");
  const [ruleSetId, setRuleSetId] = useState("riichi/yonma");
  const [presetId, setPresetId] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const selectedRuleSet = catalog.data?.rule_sets.find((rs) => rs.id === ruleSetId);
  const presets = selectedRuleSet?.presets ?? [];

  const submit = async () => {
    if (!token) return;
    setError(null);
    setLoading(true);
    try {
      const config: Record<string, unknown> = {};
      if (presetId) config.preset = { id: presetId, revision: 1 };
      const room = await gameApi.createRoom(
        { name: name || "新房间", visibility: "public", rules: { rule_set_id: ruleSetId, config } },
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
        <div style={{ color: "var(--red-soft)", marginBottom: 14, fontSize: 13 }}>无法加载规则：{apiFailure(catalog.error).message}</div>
      )}
      {error && <div style={{ color: "var(--red-soft)", marginBottom: 14, fontSize: 13 }}>{error}</div>}
      <div style={label}>房间名称</div>
      <input style={field} placeholder="输入房间名称" value={name} onChange={(e) => setName(e.target.value)} onKeyDown={(e) => e.key === "Enter" && submit()} />
      <div style={label}>规则</div>
      <select style={field} value={ruleSetId} onChange={(e) => { setRuleSetId(e.target.value); setPresetId(""); }}>
        {catalog.data?.rule_sets.map((rs) => (
          <option key={rs.id} value={rs.id}>{rs.name}（{rs.seat_count}人）</option>
        )) ?? <option value="riichi/yonma">四人麻将</option>}
      </select>
      {presets.length > 0 && (
        <>
          <div style={label}>预设（可选）</div>
          <select style={field} value={presetId} onChange={(e) => setPresetId(e.target.value)}>
            <option value="">默认规则</option>
            {presets.map((p) => (
              <option key={p.id} value={p.id}>{p.display_name}</option>
            ))}
          </select>
        </>
      )}
      <div style={{ display: "flex", gap: 10, marginTop: 8 }}>
        <Button variant="pink" size="lg" onClick={submit} disabled={loading}>创建</Button>
        <Button variant="ghost" size="lg" onClick={() => navigateTo({ kind: "lobby" })}>取消</Button>
      </div>
    </div>
  );
}
