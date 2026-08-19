import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { GameCommandName, MatchView } from "../types";
import { tablePreviewView } from "./tablePreviewData";
import { SichuanPhaseOverlay } from "./SichuanPhase";

describe("四川定缺阶段", () => {
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

  it("收到 awaiting_dingque 时立即显示面板，四家定完前不展示花色", () => {
    const onCommand = vi.fn<(name: GameCommandName, payload?: unknown) => void>();
    const view = sichuanView({ kind: "awaiting_dingque" });
    render(view, onCommand);

    expect(container.querySelector('[aria-label="定缺"]')).not.toBeNull();
    expect(container.textContent).toContain("定缺");
    click("万");
    click("确认");
    expect(onCommand).toHaveBeenCalledWith("sichuan.ding_que", { suit: "man" });
    expect(container.textContent).not.toContain("等待其他人完成定缺");

    render(
      sichuanView({ kind: "awaiting_dingque" }, {
        dingque_submitted_seats: [0],
      }),
      onCommand,
    );
    expect(container.textContent).toContain("等待其他人完成定缺");
  });

  it("换牌阶段使用明确提示，动画阶段不叠加额外文字", () => {
    const onCommand = vi.fn<(name: GameCommandName, payload?: unknown) => void>();
    render(sichuanView({ kind: "awaiting_exchange" }), onCommand);
    expect(container.textContent).toContain("请选择3张同花色牌换牌");
    expect(container.textContent).not.toContain("逆时针");

    render(
      sichuanView({ kind: "awaiting_exchange" }, {
        exchange_submitted_seats: [0],
      }),
      onCommand,
    );
    expect(container.textContent).toContain("等待其他人选择换牌");

    render(sichuanView({ kind: "awaiting_exchange_animation" }), onCommand);
    expect(container.textContent).toBe("");
  });

  function render(
    view: MatchView,
    onCommand: (name: GameCommandName, payload?: unknown) => void,
  ) {
    act(() =>
      root.render(
        <SichuanPhaseOverlay
          view={view}
          openingPhase="play"
          onCommand={onCommand}
          onConfirmExchange={() => {}}
          exchangeLocallySubmitted={false}
          exchangeAnimationDone
        />,
      ),
    );
  }

  function click(label: string) {
    const button = Array.from(container.querySelectorAll("button")).find(
      (candidate) => candidate.textContent === label,
    );
    expect(button).not.toBeUndefined();
    act(() => button!.click());
  }
});

function sichuanView(
  phase: MatchView["phase"],
  overrides: Partial<MatchView> = {},
): MatchView {
  return {
    ...tablePreviewView,
    variant_kind: "sichuan",
    phase,
    players: tablePreviewView.players.map((player) => ({
      ...player,
      que_suit: undefined,
    })),
    exchange_submitted_seats: [],
    exchange_animation_played_seats: [],
    dingque_submitted_seats: [],
    ...overrides,
  };
}
