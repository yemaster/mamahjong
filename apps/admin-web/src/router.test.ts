// @vitest-environment jsdom

import { beforeEach, describe, expect, it } from "vitest";
import { router } from "./router";

describe("admin router", () => {
  beforeEach(() => window.history.replaceState(null, "", "/admin/"));

  it("keeps navigation under the admin base path", async () => {
    await router.push({ name: "users" });
    expect(window.location.pathname).toBe("/admin/users");
  });

  it("supports encoded detail identifiers", async () => {
    await router.push({ name: "match-detail", params: { matchId: "match/测试" } });
    expect(router.currentRoute.value.params.matchId).toBe("match/测试");
  });
});
