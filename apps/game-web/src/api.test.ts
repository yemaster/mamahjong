import { afterEach, describe, expect, it, vi } from "vitest";
import { gameApi, SESSION_INVALID_EVENT } from "./api";

describe("gameApi", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("sends the login token with protected room reads", async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      json: async () => ({ rooms: [] }),
    });
    vi.stubGlobal("fetch", fetchMock);

    await gameApi.rooms("session-token");
    await gameApi.getRoom("room-id", "session-token");
    await gameApi.activity("session-token");

    for (const [, init] of fetchMock.mock.calls) {
      const headers = init.headers as Headers;
      expect(headers.get("authorization")).toBe("Bearer session-token");
    }
  });

  it("notifies the app when an authenticated session is revoked", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: false,
        status: 401,
        json: async () => ({
          code: "auth.invalid_session",
          message: "session is invalid",
        }),
      }),
    );
    const invalidated = vi.fn();
    window.addEventListener(SESSION_INVALID_EVENT, invalidated);

    await expect(gameApi.me("old-session")).rejects.toMatchObject({
      status: 401,
      code: "auth.invalid_session",
    });
    expect(invalidated).toHaveBeenCalledOnce();

    window.removeEventListener(SESSION_INVALID_EVENT, invalidated);
  });
});
