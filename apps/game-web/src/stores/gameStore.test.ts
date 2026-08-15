import { afterEach, describe, expect, it } from "vitest";
import type { MatchView } from "../types";
import { useGameStore } from "./gameStore";

describe("对局状态", () => {
  afterEach(() => useGameStore.getState().reset());

  it("旧响应缺少计时数组时仍能进入对局", () => {
    useGameStore.getState().setMatchView({
      id: "match_test",
      version: 1,
      clocks: undefined,
    } as unknown as MatchView);

    expect(useGameStore.getState().matchView?.id).toBe("match_test");
    expect(useGameStore.getState().clocks.size).toBe(0);
  });

  it("忽略迟到的旧版本和重复快照，防止牌桌状态来回重绘", () => {
    const latest = {
      id: "match_test",
      version: 8,
      clocks: [],
    } as unknown as MatchView;
    useGameStore.getState().setMatchView(latest);
    useGameStore.getState().setMatchView({
      ...latest,
      version: 7,
    });
    useGameStore.getState().setMatchView({
      ...latest,
      version: 8,
    });

    expect(useGameStore.getState().matchView).toBe(latest);
    expect(useGameStore.getState().version).toBe(8);
  });
});
