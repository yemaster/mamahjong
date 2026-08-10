import { useEffect, useRef, useState } from "react";

/**
 * 跳局用的下拉。
 *
 * 不用原生 `<select>`：选项里要写的不止一行字，而是「局」当标题、下面跟四家的
 * 点数变化，原生下拉塞不进这些东西，样式也是浏览器说了算，跟牌桌那身皮对不上。
 * 所以整块自己画：斜切金边、深蓝底、亮金标题，和宝牌面板同一套语言。
 */

export interface HandDelta {
  seat: number;
  nickname: string;
  delta: number;
}

export interface HandOption {
  handIndex: number;
  title: string;
  /** 这一局四家各自的点数变化，座次顺序。 */
  deltas: HandDelta[];
}

export function HandPicker({
  options,
  value,
  onSelect,
}: {
  options: HandOption[];
  value: number;
  onSelect: (handIndex: number) => void;
}) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement | null>(null);
  const currentRef = useRef<HTMLButtonElement | null>(null);
  const current = options.find((option) => option.handIndex === value);

  /* 点到别处、或者按 Esc 就收起来。 */
  useEffect(() => {
    if (!open) return;
    const onPointerDown = (event: PointerEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) setOpen(false);
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    window.addEventListener("pointerdown", onPointerDown);
    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("pointerdown", onPointerDown);
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [open]);

  /* 半庄十几局，展开时先滚到当前那一局，省得每次从东一局往下找。 */
  useEffect(() => {
    if (open) currentRef.current?.scrollIntoView({ block: "nearest" });
  }, [open]);

  return (
    <div className="replay-picker" ref={rootRef}>
      <button
        type="button"
        className={`replay-picker__trigger${open ? " is-open" : ""}`}
        onClick={() => setOpen((value) => !value)}
        aria-haspopup="listbox"
        aria-expanded={open}
      >
        <span>{current?.title ?? "—"}</span>
        <i className="replay-picker__caret" aria-hidden="true" />
      </button>

      {open && (
        <div className="replay-picker__panel" role="listbox" aria-label="选择局">
          {options.map((option) => {
            const isCurrent = option.handIndex === value;
            return (
              <button
                key={option.handIndex}
                ref={isCurrent ? currentRef : undefined}
                type="button"
                role="option"
                aria-selected={isCurrent}
                className={`replay-picker__item${isCurrent ? " is-current" : ""}`}
                onClick={() => {
                  setOpen(false);
                  onSelect(option.handIndex);
                }}
              >
                <span className="replay-picker__item-title">{option.title}</span>
                <span className="replay-picker__item-deltas">
                  {option.deltas.map((entry) => (
                    <span
                      key={entry.seat}
                      className={`replay-picker__delta ${deltaTone(entry.delta)}`}
                    >
                      <i>{entry.nickname}</i>
                      <b>{formatHandDelta(entry.delta)}</b>
                    </span>
                  ))}
                </span>
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}

function deltaTone(delta: number): string {
  if (delta > 0) return "is-gain";
  if (delta < 0) return "is-loss";
  return "is-flat";
}

/** 一局之内的点数变化，零写作 `±0`，跟正负号对齐排版。 */
export function formatHandDelta(delta: number): string {
  if (delta > 0) return `+${delta}`;
  if (delta < 0) return `${delta}`;
  return "±0";
}
