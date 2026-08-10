import type { MatchAssistSettings } from "./matchAssist";

interface MatchAssistControlsProps {
  settings: MatchAssistSettings;
  onChange: (settings: MatchAssistSettings) => void;
}

const CONTROLS: {
  key: keyof MatchAssistSettings;
  label: string;
  description: string;
}[] = [
  { key: "autoSort", label: "理", description: "自动理牌" },
  { key: "autoWin", label: "和", description: "自动和牌" },
  { key: "skipCalls", label: "鸣", description: "跳过吃碰杠" },
  { key: "autoTsumogiri", label: "切", description: "自动摸切" },
];

export function MatchAssistControls({
  settings,
  onChange,
}: MatchAssistControlsProps) {
  return (
    <aside className="match-assist-controls" aria-label="快捷操作">
      {CONTROLS.map((control) => {
        const active = settings[control.key];
        return (
          <button
            key={control.key}
            type="button"
            className={active ? "is-active" : ""}
            aria-pressed={active}
            aria-label={`${control.description}${active ? "已开启" : "已关闭"}`}
            title={control.description}
            onClick={() =>
              onChange({
                ...settings,
                [control.key]: !active,
              })
            }
          >
            {control.label}
          </button>
        );
      })}
    </aside>
  );
}
