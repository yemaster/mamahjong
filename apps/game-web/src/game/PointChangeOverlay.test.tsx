import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { MatchView } from "../types";
import { PointChangeOverlay } from "./PointChangeOverlay";
import { tablePreviewView } from "./tablePreviewData";

vi.mock("../audio/sfx", () => ({
  playSfx: vi.fn(),
  SCORE_CHANGE_SFX: "/score.mp3",
}));

describe("四川流局点数动画", () => {
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

  it("只显示查花猪/查大叫的增减，不重复此前已播放的胡牌分", () => {
    const view: MatchView = {
      ...structuredClone(tablePreviewView),
      variant_kind: "sichuan",
      hand_settlement: {
        reason: "exhaustive_draw",
        tenpai_seats: [],
        point_deltas: [9000, -9000, 0, 0],
        points_before: [21000, 39000, 25000, 25000],
        points_after: [30000, 30000, 25000, 25000],
        winners: [],
        played_seats: [],
        confirm_remaining_ms: null,
        confirmed_seats: [],
        from_seat: null,
        ura_dora_indicators: [],
        que: {
          flower_pigs: [],
          tenpai: [0],
          noten: [1],
          deltas: [1000, -1000, 0, 0],
        },
      },
    };

    act(() =>
      root.render(
        <PointChangeOverlay
          view={view}
          pointDeltas={view.hand_settlement!.que!.deltas}
        />,
      ),
    );

    expect(container.textContent).toContain("+1,000");
    expect(container.textContent).toContain("-1,000");
    expect(container.textContent).not.toContain("+9,000");
    expect(container.textContent).not.toContain("-9,000");
  });
});
