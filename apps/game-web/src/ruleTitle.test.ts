import { describe, expect, it } from "vitest";
import { roomRuleTitle } from "./ruleTitle";
import type { RoomView } from "./types";

function room(over: Partial<RoomView>): RoomView {
  return {
    schema: "room.v1",
    id: "room_1",
    version: 1,
    owner_user_id: "user_1",
    name: "好友房间",
    visibility: "private",
    lifecycle: "waiting",
    seat_count: 4,
    variant_kind: "riichi",
    rule_name: "标准规则",
    rule_snapshot: { rule_set_id: "riichi/yonma", config: {} },
    members: [],
    active_match_id: null,
    ...over,
  };
}

describe("roomRuleTitle", () => {
  it("spells out seats, length and rule name for riichi", () => {
    const title = roomRuleTitle(
      room({
        rule_name: "A规",
        rule_snapshot: {
          rule_set_id: "riichi/yonma",
          config: { variant: "yonma", match_rules: { length: "hanchan" } },
        },
      }),
    );

    expect(title).toBe("立直麻将 · 四人南 · A规");
  });

  it("writes 三人东 for a sanma east-only table", () => {
    const title = roomRuleTitle(
      room({
        rule_snapshot: {
          rule_set_id: "riichi/sanma",
          config: { variant: "sanma", match_rules: { length: "east_only" } },
        },
      }),
    );

    expect(title).toBe("立直麻将 · 三人东 · 标准规则");
  });

  it("writes the mode for impact and drops the standard rule name", () => {
    const title = roomRuleTitle(
      room({
        variant_kind: "impact",
        rule_snapshot: {
          rule_set_id: "impact/yonma",
          config: { mode: "blind" },
        },
      }),
    );

    expect(title).toBe("冲击麻将 · 瞎子麻将");
  });

  it("keeps the rule name for impact once it has been changed", () => {
    const title = roomRuleTitle(
      room({
        variant_kind: "impact",
        rule_name: "自定义规则",
        rule_snapshot: {
          rule_set_id: "impact/yonma",
          config: { mode: "blind" },
        },
      }),
    );

    expect(title).toBe("冲击麻将 · 瞎子麻将 · 自定义规则");
  });
});
