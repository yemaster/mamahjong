import { afterEach, describe, expect, it, vi } from "vitest";
import {
  RETURN_TO_SPLASH_EVENT,
  returnToSplashForLogin,
  useAuthStore,
} from "./authStore";

describe("大厅退出流程", () => {
  afterEach(() => {
    useAuthStore.getState().logout();
    vi.restoreAllMocks();
  });

  it("先通知根场景返回加载页，不在大厅内立即清除登录状态", () => {
    const listener = vi.fn();
    window.addEventListener(RETURN_TO_SPLASH_EVENT, listener);
    useAuthStore.getState().setToken("测试令牌");

    returnToSplashForLogin();

    expect(listener).toHaveBeenCalledOnce();
    expect(useAuthStore.getState().token).toBe("测试令牌");
    window.removeEventListener(RETURN_TO_SPLASH_EVENT, listener);
  });
});
