import { Delete } from "lucide-react";
import type { MouseEvent } from "react";

const DIGITS = ["1", "2", "3", "4", "5", "6", "7", "8", "9"] as const;

/*
 * 模拟九键数字键盘，没有实体键盘的设备点按即可输入房间号。
 * 按键按下时不抢输入框焦点，实体键盘还能接着用。
 */
export function RoomIdKeypad({
  value,
  onChange,
}: {
  value: string;
  onChange: (value: string) => void;
}) {
  const keepInputFocus = (event: MouseEvent<HTMLButtonElement>) =>
    event.preventDefault();

  return (
    <div className="game-lobby__keypad" aria-label="房间号键盘">
      {DIGITS.map((digit) => (
        <button
          key={digit}
          type="button"
          className="game-lobby__keypad-key"
          onMouseDown={keepInputFocus}
          onClick={() => onChange(`${value}${digit}`.slice(0, 6))}
        >
          {digit}
        </button>
      ))}
      <button
        type="button"
        className="game-lobby__keypad-key game-lobby__keypad-key--action"
        aria-label="删除一位"
        onMouseDown={keepInputFocus}
        onClick={() => onChange(value.slice(0, -1))}
      >
        <Delete size={28} aria-hidden="true" />
      </button>
      <button
        type="button"
        className="game-lobby__keypad-key"
        onMouseDown={keepInputFocus}
        onClick={() => onChange(`${value}0`.slice(0, 6))}
      >
        0
      </button>
      <button
        type="button"
        className="game-lobby__keypad-key game-lobby__keypad-key--action"
        aria-label="清空房间号"
        onMouseDown={keepInputFocus}
        onClick={() => onChange("")}
      >
        清除
      </button>
    </div>
  );
}
