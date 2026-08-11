// @vitest-environment jsdom

import { flushPromises, mount } from "@vue/test-utils";
import PrimeVue from "primevue/config";
import { beforeEach, describe, expect, it, vi } from "vitest";
import LoginPage from "./LoginPage.vue";

const { replace, signIn, bootstrap } = vi.hoisted(() => ({
  replace: vi.fn(),
  signIn: vi.fn(),
  bootstrap: vi.fn(),
}));

vi.mock("vue-router", () => ({
  useRouter: () => ({ replace }),
}));

vi.mock("../session", () => ({
  useAdminSession: () => ({ signIn }),
}));

vi.mock("../api", () => ({
  adminApi: { bootstrap },
}));

describe("LoginPage", () => {
  beforeEach(() => {
    replace.mockReset();
    signIn.mockReset().mockResolvedValue(undefined);
    bootstrap.mockReset().mockResolvedValue({
      schema: "admin_session_bootstrap.v1",
      enabled: true,
      login_csrf: "csrf-login",
    });
  });

  async function renderPage() {
    const wrapper = mount(LoginPage, {
      global: { plugins: [[PrimeVue, { ripple: false }]] },
    });
    await flushPromises();
    return wrapper;
  }

  it("submits the entered credentials", async () => {
    const wrapper = await renderPage();
    await wrapper.get('input[name="loginName"]').setValue("admin");
    await wrapper.get('input[name="password"]').setValue("abc123456");
    await wrapper.get("form").trigger("submit");
    await flushPromises();

    expect(signIn).toHaveBeenCalledWith("admin", "abc123456", "csrf-login");
    expect(replace).toHaveBeenCalledWith({ name: "overview" });
  });

  it("shows validation errors without submitting empty fields", async () => {
    const wrapper = await renderPage();
    await wrapper.get("form").trigger("submit");

    expect(signIn).not.toHaveBeenCalled();
    expect(wrapper.text()).toContain("请输入账号");
    expect(wrapper.text()).toContain("请输入密码");
  });
});
