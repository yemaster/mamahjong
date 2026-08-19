import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { MatchView, SichuanWinView } from "../types";
import { tablePreviewView } from "./tablePreviewData";

vi.mock("../audio/sfx", () => ({
  playSfx: vi.fn(),
  SCORE_CHANGE_SFX: "/score.mp3",
}));

import { SichuanWinOverlay } from "./SichuanWinOverlay";

describe("四川胡牌即时点数动画", () => {
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

  it("按胡牌增量显示四家统一点数卡，而不是占位文字", () => {
    const view: MatchView = {
      ...structuredClone(tablePreviewView),
      variant_kind: "sichuan",
    };
    const win: SichuanWinView = {
      id: 1,
      seat: 0,
      is_tsumo: true,
      payer: null,
      chankan: false,
      winning_tile: { id: 9999, code: "5p" },
      deltas: [6000, -2000, -2000, -2000],
    };
    const onReveal = vi.fn();
    const onFinished = vi.fn();

    act(() =>
      root.render(
        <SichuanWinOverlay
          view={view}
          win={win}
          onReveal={onReveal}
          onFinished={onFinished}
        />,
      ),
    );

    expect(container.querySelectorAll(".match-point-change__card")).toHaveLength(4);
    expect(container.textContent).toContain("+6,000");
    expect(container.textContent).toContain("-2,000");
    expect(container.textContent).not.toContain("胡牌动画");
    expect(container.textContent).not.toContain("杠点");
    expect(container.querySelector(".match-point-change__banner")).toBeNull();
    expect(onReveal).not.toHaveBeenCalled();
    expect(onFinished).not.toHaveBeenCalled();
  });

  it("荣和不显示额外的小信息框", () => {
    const view: MatchView = {
      ...structuredClone(tablePreviewView),
      variant_kind: "sichuan",
    };
    const win: SichuanWinView = {
      id: 2,
      seat: 0,
      is_tsumo: false,
      payer: 1,
      chankan: false,
      winning_tile: { id: 9999, code: "5p" },
      deltas: [6000, -6000, 0, 0],
    };

    act(() =>
      root.render(
        <SichuanWinOverlay
          view={view}
          win={win}
          onReveal={() => {}}
          onFinished={() => {}}
        />,
      ),
    );

    expect(container.querySelector(".match-point-change__banner")).toBeNull();
  });
});
