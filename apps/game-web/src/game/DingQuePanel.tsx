/**
 * 四川麻将定缺面板：游戏画面正中弹出，三个56px大字按钮「万」「筒」「条」+确认按钮。
 * 选中哪个大字就高亮那个，点确认才提交。
 */
import { useState } from "react";

const DINGQUE_OPTIONS = [
  { value: "man", label: "万" },
  { value: "pin", label: "筒" },
  { value: "sou", label: "条" },
];

export function DingQuePanel({
  onSelect,
  disabled = false,
}: {
  onSelect: (suit: string) => void;
  /** 后端已通知定缺，但本地换牌模型还在飞时先展示、暂不允许提交。 */
  disabled?: boolean;
}) {
  const [selected, setSelected] = useState<string | null>(null);

  return (
    <div className="match-dingque-panel" aria-label="定缺">
      <div className="match-dingque-panel__title">
        <span>定缺</span>
      </div>
      <div className="match-dingque-panel__options">
        {DINGQUE_OPTIONS.map((option) => (
          <button
            key={option.value}
            type="button"
            className={`match-dingque-panel__suit${
              selected === option.value ? " is-selected" : ""
            }`}
            disabled={disabled}
            onClick={() => setSelected(option.value)}
          >
            {option.label}
          </button>
        ))}
      </div>
      <button
        type="button"
        className="match-brush-button"
        disabled={disabled || !selected}
        onClick={() => {
          if (selected) onSelect(selected);
        }}
      >
        确认
      </button>
    </div>
  );
}
