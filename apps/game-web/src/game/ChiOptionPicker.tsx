import type { ChiOption } from "./chiOptions";
import { TilePlate } from "./TilePlate";

/**
 * 吃牌方案选择框。
 *
 * 上家那张牌能用不止一组手牌吃的时候，浮在主视角手牌正上方的一块小板：左端一个
 * 「吃」字，中间一组组摆出要拿出去的两张牌，末端一颗「返回」。
 *
 * 样式跟听牌提示同一套：金边、深蓝底、两个斜切角；牌的大小取的是主视角那块听牌
 * 提示框（`.match-wait-assist__panel`）那一档，两块板都浮在手牌正上方，牌不一样
 * 大只会显得是两套界面。
 *
 * 一组之内两张牌贴着放、组与组之间拉开，靠的就是这个距离差让人一眼看出哪两张是
 * 一组的（见 `docs/game-table-visual-spec.md` 的「吃牌方案选择」一节）。
 */
export function ChiOptionPicker({
  options,
  onSelect,
  onCancel,
}: {
  options: ChiOption[];
  onSelect: (tileIds: [number, number]) => void;
  onCancel: () => void;
}) {
  if (options.length === 0) return null;

  return (
    <div className="match-chi-picker" aria-label="选择吃牌方案">
      <span className="match-chi-picker__label" aria-hidden="true">
        吃
      </span>
      <div className="match-chi-picker__options">
        {options.map((option) => (
          <button
            key={option.key}
            type="button"
            className="match-chi-picker__option"
            aria-label={`用${option.tiles.map(tileLabel).join("和")}吃`}
            onClick={() => onSelect(option.tileIds)}
          >
            {option.tiles.map((tile) => (
              <TilePlate key={tile.id} code={tile.code} />
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

const SUIT_NAMES: Record<string, string> = { m: "万", p: "饼", s: "条" };

/** 读屏用的牌名：`0s` 是赤五条，其余按「几万/几饼/几条」念。 */
function tileLabel({ code }: { code: string }): string {
  const suit = SUIT_NAMES[code.slice(-1)] ?? "";
  const number = Number(code.slice(0, -1));
  return number === 0 ? `赤五${suit}` : `${number}${suit}`;
}
