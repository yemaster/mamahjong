import { describe, expect, it } from "vitest";
import { navigateToActivity } from "./activity";

describe("恢复当前活动", () => {
  it("进行中的对局会直接跳回牌桌", () => {
    window.location.hash = "#lobby";

    const resumed = navigateToActivity({
      schema: "user_activity.v1",
      kind: "game",
      room_id: "123456",
      match_id: "match_test",
      ticket_id: null,
    });

    expect(resumed).toBe(true);
    expect(window.location.hash).toBe("#game/match_test");
  });

  it("空闲状态不改变页面", () => {
    window.location.hash = "#lobby";

    const resumed = navigateToActivity({
      schema: "user_activity.v1",
      kind: "idle",
      room_id: null,
      match_id: null,
      ticket_id: null,
    });

    expect(resumed).toBe(false);
    expect(window.location.hash).toBe("#lobby");
  });
});
