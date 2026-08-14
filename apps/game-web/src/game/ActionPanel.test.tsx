import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { GameCommandName, ImpactRuleConfig, MatchView } from "../types";
import { ActionPanel } from "./ActionPanel";
import { tablePreviewView } from "./tablePreviewData";

describe("ActionPanel 冲击麻将响应", () => {
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

  it("亮子麻将发送荣和、唯一吃法和取消的正确指令", () => {
    const onCommand = vi.fn<(name: GameCommandName, payload?: unknown) => void>();
    renderPanel(
      impactView("bright", [
        { kind: "ron" },
        { kind: "chi", tile_ids: [1000, 1001] },
      ]),
      onCommand,
    );

    clickButton("荣和");
    clickButton("吃");
    clickButton("取消");

    expect(onCommand.mock.calls).toEqual([
      ["impact.ron"],
      ["impact.chi", { tile_ids: [1000, 1001] }],
      ["impact.pass"],
    ]);
  });

  it("多个吃法先打开选择器，不擅自替玩家挑牌", () => {
    const onCommand = vi.fn<(name: GameCommandName, payload?: unknown) => void>();
    const onChiSelectingChange = vi.fn<(selecting: boolean) => void>();
    renderPanel(
      impactView("bright", [
        { kind: "chi", tile_ids: [1000, 1001] },
        { kind: "chi", tile_ids: [1003, 1004] },
      ]),
      onCommand,
      onChiSelectingChange,
    );

    clickButton("吃");

    expect(onChiSelectingChange).toHaveBeenCalledWith(true);
    expect(onCommand).not.toHaveBeenCalled();
  });

  it("不根据财神和规则模式二次过滤后端下发的响应", () => {
    const onCommand = vi.fn<(name: GameCommandName, payload?: unknown) => void>();
    const view = impactView("blind", [
      { kind: "ron" },
      { kind: "chi", tile_ids: [1000, 1001] },
    ]);
    view.joker_code = "2m";
    renderPanel(view, onCommand);

    clickButton("荣和");
    clickButton("吃");
    expect(onCommand.mock.calls).toEqual([
      ["impact.ron"],
      ["impact.chi", { tile_ids: [1000, 1001] }],
    ]);
  });

  function renderPanel(
    view: MatchView,
    onCommand: (name: GameCommandName, payload?: unknown) => void,
    onChiSelectingChange = vi.fn(),
  ) {
    act(() =>
      root.render(
        <ActionPanel
          view={view}
          onCommand={onCommand}
          riichiSelecting={false}
          onRiichiSelectingChange={() => {}}
          onChiSelectingChange={onChiSelectingChange}
        />,
      ),
    );
  }

  function button(label: string): HTMLButtonElement | null {
    return (
      Array.from(container.querySelectorAll("button")).find(
        (candidate) => candidate.textContent === label,
      ) ?? null
    );
  }

  function clickButton(label: string) {
    const target = button(label);
    expect(target).not.toBeNull();
    act(() => target!.click());
  }
});

function impactView(
  mode: ImpactRuleConfig["mode"],
  availableReactions: MatchView["available_reactions"],
): MatchView {
  return {
    ...tablePreviewView,
    variant_kind: "impact",
    phase: { kind: "awaiting_responses", trigger_seat: 3 },
    impact_rules: impactRules(mode),
    available_reactions: availableReactions,
  };
}

function impactRules(mode: ImpactRuleConfig["mode"]): ImpactRuleConfig {
  const bright = mode === "bright";
  return {
    mode,
    match_rules: { thinking_time: { base_seconds: 5, reserve_seconds: 20 } },
    kan: {
      added_kan_single_payer: !bright,
      indicator_pon_counts_as_kan: !bright,
      first_round_repeat_discard: !bright,
      four_identical_discards_as_kan: !bright,
      pon_with_few_tiles_as_kan: !bright,
    },
    special: { seven_gaps: false },
    all_in: {
      eleven_honor_streak: true,
      all_honors: !bright,
      pure_flush_no_joker: !bright,
      single_wait: !bright,
      three_kans: !bright,
      four_jokers: true,
      pure_seven_pairs: !bright,
      last_tile: !bright,
      blessing: true,
    },
  };
}
