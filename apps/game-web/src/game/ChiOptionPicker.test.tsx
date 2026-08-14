import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ChiOptionPicker } from "./ChiOptionPicker";
import type { ChiOption } from "./chiOptions";

describe("ChiOptionPicker", () => {
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

  it("选择方案时原样交回该组实体牌 id", () => {
    const onSelect = vi.fn<(tileIds: [number, number]) => void>();
    const options: ChiOption[] = [
      {
        tileIds: [12, 34],
        tiles: [
          { id: 12, code: "3m" },
          { id: 34, code: "5m" },
        ],
        key: "3m+5m",
      },
    ];
    act(() =>
      root.render(
        <ChiOptionPicker
          options={options}
          onSelect={onSelect}
          onCancel={() => {}}
        />,
      ),
    );

    const option = container.querySelector<HTMLButtonElement>(
      ".match-chi-picker__option",
    );
    act(() => option!.click());

    expect(onSelect).toHaveBeenCalledWith([12, 34]);
  });

  it("返回只关闭选择器，不选择任何方案", () => {
    const onSelect = vi.fn();
    const onCancel = vi.fn();
    act(() =>
      root.render(
        <ChiOptionPicker
          options={[
            {
              tileIds: [1, 2],
              tiles: [
                { id: 1, code: "1s" },
                { id: 2, code: "2s" },
              ],
              key: "1s+2s",
            },
          ]}
          onSelect={onSelect}
          onCancel={onCancel}
        />,
      ),
    );

    const back = Array.from(container.querySelectorAll("button")).find(
      (candidate) => candidate.textContent === "返回",
    );
    act(() => back!.click());

    expect(onCancel).toHaveBeenCalledOnce();
    expect(onSelect).not.toHaveBeenCalled();
  });
});
