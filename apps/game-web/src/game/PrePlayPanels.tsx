/** 一张数牌的花色码：`1m`→`m`，`3p`→`p`，`7s`→`s`；非数牌返回 `null`。 */
export function tileSuitCode(code: string): "m" | "p" | "s" | null {
  const suit = code.slice(-1);
  return suit === "m" || suit === "p" || suit === "s" ? suit : null;
}

/**
 * 几选一的通用面板：定缺选花色用它，以后哪套规则要「报一个选项」也直接搬。
 * 选项本身就是一颗颗操作按钮，点下去即提交，不另设确定。
 */
export function ChoicePanel({
  title,
  options,
  onSelect,
}: {
  title: string;
  options: { value: string; label: string }[];
  onSelect: (value: string) => void;
}) {
  return (
    <div className="match-chi-picker" aria-label={title}>
      <span className="match-chi-picker__label">{title}</span>
      <div className="match-chi-picker__options">
        {options.map((option) => (
          <button
            key={option.value}
            type="button"
            className="match-brush-button"
            onClick={() => onSelect(option.value)}
          >
            {option.label}
          </button>
        ))}
      </div>
    </div>
  );
}
