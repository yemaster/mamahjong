import type { LucideIcon } from "lucide-react";

/**
 * 大厅右下角功能区的按钮。
 *
 * 一个圆按钮，图标和文字都装在圆里：上图标、下一行词。素得比中间的主菜单按钮低
 * 一档，不套斜切外框也不叠内层板——主菜单那身皮留给主操作，功能入口不抢戏。
 * 圆的尺寸固定、里头是一列居中的网格，几颗并排时圆心和中轴自然对齐。
 */
export function LobbyFunctionButton({
  icon: Icon,
  label,
  onClick,
}: {
  icon: LucideIcon;
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      className="game-lobby__function"
      onClick={onClick}
      aria-label={label}
    >
      <span className="game-lobby__function-icon" aria-hidden="true">
        <Icon />
      </span>
      <b className="game-lobby__function-text">{label}</b>
    </button>
  );
}
