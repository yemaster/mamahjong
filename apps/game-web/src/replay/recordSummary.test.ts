import { describe, expect, it } from "vitest";
import { matchRecordTitle, recordTitleParts } from "./recordSummary";
import type { MatchRecord } from "./recordTypes";

describe("matchRecordTitle", () => {
  it("把麻将种类排在人数前面", () => {
    expect(
      matchRecordTitle({
        friend_match: true,
        rule_family: "riichi",
        variant: "yonma",
        match_length: "hanchan",
        rule_name: "自定义规则",
      }),
    ).toBe("好友对战 · 立直麻将 · 四人南 · 自定义规则");
  });

  it("旧牌谱没记麻将种类就少写这一段", () => {
    expect(
      matchRecordTitle({
        friend_match: null,
        variant: "sanma",
        match_length: "east_only",
        rule_name: "ML规则",
      }),
    ).toBe("三人东 · ML规则");
  });

  it("段位匹配不重复写规则名", () => {
    expect(
      matchRecordTitle({
        friend_match: false,
        rule_family: "riichi",
        variant: "yonma",
        match_length: "hanchan",
        rule_name: "标准规则",
      }),
    ).toBe("段位匹配 · 立直麻将 · 四人南");
  });

  it("从牌谱本体的规则集 ID 里取麻将种类", () => {
    const record = {
      friend_match: true,
      rule_snapshot: {
        rule_set_id: "riichi/yonma",
        config: { variant: "yonma", match_rules: { length: "hanchan" } },
      },
      rule_name: "A规",
    } as unknown as MatchRecord;

    expect(recordTitleParts(record).rule_family).toBe("riichi");
    expect(matchRecordTitle(recordTitleParts(record))).toBe(
      "好友对战 · 立直麻将 · 四人南 · A规",
    );
  });
});
