// @vitest-environment jsdom

import { describe, expect, it, vi } from "vitest";
import { navigate } from "./routing";

describe("admin navigation", () => {
  it("uses stable paths under the admin base path", () => {
    const listener = vi.fn();
    window.addEventListener("popstate", listener);

    navigate("/users");

    expect(window.location.pathname).toBe("/admin/users");
    expect(listener).toHaveBeenCalledOnce();
  });
});
