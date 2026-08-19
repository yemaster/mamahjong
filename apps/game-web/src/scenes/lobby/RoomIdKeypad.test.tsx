import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { RoomIdKeypad } from "./RoomIdKeypad";

describe("RoomIdKeypad", () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  const key = (label: string) =>
    Array.from(
      container.querySelectorAll<HTMLButtonElement>(
        ".game-lobby__keypad-key",
      ),
    ).find(
      (candidate) =>
        candidate.textContent === label ||
        candidate.getAttribute("aria-label") === label,
    );

  it("点数字按顺序追加，满六位后不再加长", () => {
    const onChange = vi.fn();
    act(() => root.render(<RoomIdKeypad value="12345" onChange={onChange} />));
    act(() => key("6")!.click());
    expect(onChange).toHaveBeenLastCalledWith("123456");

    act(() =>
      root.render(<RoomIdKeypad value="123456" onChange={onChange} />),
    );
    act(() => key("7")!.click());
    expect(onChange).toHaveBeenLastCalledWith("123456");
  });

  it("删除去掉末位，清除整个置空", () => {
    const onChange = vi.fn();
    act(() => root.render(<RoomIdKeypad value="42" onChange={onChange} />));

    act(() => key("删除一位")!.click());
    expect(onChange).toHaveBeenLastCalledWith("4");

    act(() => key("清空房间号")!.click());
    expect(onChange).toHaveBeenLastCalledWith("");
  });
});
