// @vitest-environment jsdom

import { afterEach, describe, expect, it, vi } from "vitest";
import { adminApi, ApiError } from "./api";

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("adminApi", () => {
  it("sends the CSRF token on state changes", async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValue(new Response(null, { status: 204 }));
    vi.stubGlobal("fetch", fetchMock);

    await adminApi.updateUserStatus("user_1", "suspended", "csrf_1");

    const [, init] = fetchMock.mock.calls[0] as [string, RequestInit];
    const headers = new Headers(init.headers);
    expect(init.method).toBe("PUT");
    expect(headers.get("x-csrf-token")).toBe("csrf_1");
    expect(JSON.parse(String(init.body))).toEqual({ status: "suspended" });
  });

  it("notifies the application when an active session expires", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        new Response(
          JSON.stringify({
            code: "admin.unauthorized",
            message: "请重新登录",
          }),
          {
            status: 401,
            headers: { "content-type": "application/json" },
          },
        ),
      ),
    );
    const listener = vi.fn();
    window.addEventListener("mamahjong-admin-unauthorized", listener);

    await expect(adminApi.users()).rejects.toBeInstanceOf(ApiError);
    expect(listener).toHaveBeenCalledOnce();
  });
});
