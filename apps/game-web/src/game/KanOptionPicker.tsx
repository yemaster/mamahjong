import type { KanOption } from "./kanOptions";
import { TilePlate } from "./TilePlate";

/** 多个暗杠/加杠候选时使用的选择框，布局与吃牌方案框保持一致。 */
export function KanOptionPicker({
  options,
  onSelect,
  onCancel,
}: {
  options: KanOption[];
  onSelect: (option: KanOption) => void;
  onCancel: () => void;
}) {
  if (options.length === 0) return null;

  return (
    <div className="match-chi-picker" aria-label="选择杠牌方案">
      <span className="match-chi-picker__label" aria-hidden="true">
        杠
      </span>
      <div className="match-chi-picker__options">
        {options.map((option) => (
          <button
            key={option.key}
            type="button"
            className="match-chi-picker__option"
            aria-label={`杠${option.tiles.map((tile) => tileLabel(tile.code)).join("")}`}
            onClick={() => onSelect(option)}
          >
            {option.tiles.map((tile, index) => (
              <TilePlate key={`${tile.id}:${index}`} code={tile.code} />
            ))}
          </button>
        ))}
      </div>
      <button
        type="button"
        className="match-chi-picker__back"
        onClick={onCancel}
      >
        返回
      </button>
    </div>
  );
}

const SUIT_NAMES: Record<string, string> = { m: "万", p: "饼", s: "条", z: "字" };

function tileLabel(code: string): string {
  const suit = SUIT_NAMES[code.slice(-1)] ?? "";
  const number = Number(code.slice(0, -1));
  return number === 0 ? `赤五${suit}` : `${number}${suit}`;
}
