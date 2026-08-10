import { describe, expect, it } from "vitest";
import type { MatchView } from "../types";
import { useGameStore } from "./gameStore";

describe("对局状态", () => {
  it("旧响应缺少计时数组时仍能进入对局", () => {
    useGameStore.getState().setMatchView({
      id: "match_test",
      version: 1,
      clocks: undefined,
    } as unknown as MatchView);

    expect(useGameStore.getState().matchView?.id).toBe("match_test");
    expect(useGameStore.getState().clocks.size).toBe(0);
  });
});
