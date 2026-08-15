import { useEffect } from "react";
import { useAudioSettings } from "../stores/audioSettingsStore";

interface SettingsPanelProps {
  onClose: () => void;
}

export function SettingsPanel({ onClose }: SettingsPanelProps) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);
  const {
    musicVolume,
    voiceVolume,
    sfxVolume,
    setMusicVolume,
    setVoiceVolume,
    setSfxVolume,
  } = useAudioSettings();

  return (
    <div
      className="match-settings-overlay"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="match-settings-panel">
        <div className="match-settings-header">
          <span>设置</span>
          <button
            type="button"
            className="match-settings-close"
            onClick={onClose}
          >
            ✕
          </button>
        </div>
        <div className="match-settings-body">
          <VolumeRow
            label="背景音"
            value={musicVolume}
            onChange={setMusicVolume}
          />
          <VolumeRow
            label="语音"
            value={voiceVolume}
            onChange={setVoiceVolume}
          />
          <VolumeRow
            label="音效"
            value={sfxVolume}
            onChange={setSfxVolume}
          />
        </div>
      </div>
    </div>
  );
}

export function VolumeRow({
  label,
  value,
  onChange,
}: {
  label: string;
  value: number;
  onChange: (v: number) => void;
}) {
  const pct = Math.round(value * 100);
  return (
    <label className="match-settings-row">
      <span className="match-settings-label">{label}</span>
      <input
        type="range"
        className="match-settings-slider"
        min={0}
        max={100}
        value={pct}
        onChange={(e) => onChange(Number(e.target.value) / 100)}
      />
      <span className="match-settings-value">{pct}%</span>
    </label>
  );
}
