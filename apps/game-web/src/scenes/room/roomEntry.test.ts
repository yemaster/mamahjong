import { describe, expect, it } from "vitest";
import type { RoomView } from "../../types";
import { matchToEnter } from "./roomEntry";

function room(activeMatchId: string | null): RoomView {
  return {
    schema: "room.v1",
    variant_kind: "riichi",
    id: "room_1",
    version: 7,
    owner_user_id: "user_1",
    name: "同好房",
    visibility: "private",
    lifecycle: activeMatchId ? "playing" : "waiting",
    seat_count: 4,
    rule_name: "标准规则",
    rule_snapshot: { rule_set_id: "riichi/yonma", config: {} },
    members: [],
    active_match_id: activeMatchId,
  };
}

describe("matchToEnter", () => {
  it("enters the match the room really reports", () => {
    expect(matchToEnter(room("match_9"), true)).toBe("match_9");
  });

  it("stays in the room when no match is running", () => {
    expect(matchToEnter(room(null), true)).toBeNull();
  });

  it("ignores a cached room that still points at the finished match", () => {
    // 投票退出后回房间：缓存里那份是开局那一刻写下的，照它跳就来回闪。
    expect(matchToEnter(room("match_9"), false)).toBeNull();
  });

  it("waits for the room before deciding anything", () => {
    expect(matchToEnter(undefined, true)).toBeNull();
  });
});
