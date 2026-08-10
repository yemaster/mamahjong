import { describe, expect, it } from "vitest";
import { openingDice } from "./OpeningSequence";

describe("对局开场骰子", () => {
  it("为同一局生成稳定且有效的两个点数", () => {
    const first = openingDice("match_test", 3);
    const second = openingDice("match_test", 3);

    expect(first).toEqual(second);
    expect(first.every((value) => value >= 1 && value <= 6)).toBe(true);
  });
});
