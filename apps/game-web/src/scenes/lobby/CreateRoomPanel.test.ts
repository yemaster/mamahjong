import { describe, expect, it } from "vitest";
import type { ImpactRuleConfig } from "../../types";
import { impactRulesForMode } from "./CreateRoomPanel";

const base: ImpactRuleConfig = {
  mode: "blind",
  match_rules: { thinking_time: { base_seconds: 5, reserve_seconds: 20 } },
  kan: {
    added_kan_single_payer: true,
    indicator_pon_counts_as_kan: true,
    first_round_repeat_discard: true,
    four_identical_discards_as_kan: true,
    pon_with_few_tiles_as_kan: true,
  },
  special: { seven_gaps: false },
  all_in: {
    eleven_honor_streak: true,
    all_honors: true,
    pure_flush_no_joker: true,
    single_wait: true,
    three_kans: true,
    four_jokers: true,
    pure_seven_pairs: true,
    last_tile: true,
    blessing: true,
  },
};

describe("impactRulesForMode", () => {
  it("亮子默认关闭杠牌附加项且只保留三项全交", () => {
    const rules = impactRulesForMode(base, "bright");

    expect(Object.values(rules.kan)).toEqual([false, false, false, false, false]);
    expect(rules.all_in).toEqual({
      eleven_honor_streak: true,
      all_honors: false,
      pure_flush_no_joker: false,
      single_wait: false,
      three_kans: false,
      four_jokers: true,
      pure_seven_pairs: false,
      last_tile: false,
      blessing: true,
    });
  });

  it("切回瞎子恢复原有全开默认", () => {
    const rules = impactRulesForMode(impactRulesForMode(base, "bright"), "blind");

    expect(Object.values(rules.kan).every(Boolean)).toBe(true);
    expect(Object.values(rules.all_in).every(Boolean)).toBe(true);
  });
});
