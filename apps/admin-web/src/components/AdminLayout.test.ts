// @vitest-environment jsdom

import { flushPromises, mount } from "@vue/test-utils";
import PrimeVue from "primevue/config";
import { createMemoryHistory, createRouter } from "vue-router";
import { beforeEach, describe, expect, it, vi } from "vitest";
import AdminLayout from "./AdminLayout.vue";

const { toastAdd, signOut } = vi.hoisted(() => ({
  toastAdd: vi.fn(),
  signOut: vi.fn(),
}));

vi.mock("primevue/usetoast", () => ({
  useToast: () => ({ add: toastAdd }),
}));

vi.mock("../session", async () => {
  const { readonly, ref } = await import("vue");
  return {
    useAdminSession: () => ({
      identity: readonly(ref({ id: "admin", nickname: "管理员", csrf_token: "csrf" })),
      signOut,
    }),
  };
});

let mobile = false;
let changeListener: ((event: MediaQueryListEvent) => void) | undefined;

describe("AdminLayout", () => {
  beforeEach(() => {
    mobile = false;
    changeListener = undefined;
    toastAdd.mockReset();
    signOut.mockReset().mockResolvedValue(undefined);
    vi.stubGlobal("matchMedia", vi.fn(() => ({
      matches: mobile,
      media: "(max-width: 1023px)",
      onchange: null,
      addEventListener: (_type: string, listener: (event: MediaQueryListEvent) => void) => { changeListener = listener; },
      removeEventListener: vi.fn(),
      addListener: vi.fn(),
      removeListener: vi.fn(),
      dispatchEvent: vi.fn(),
    })));
  });

  async function renderLayout() {
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [
        { path: "/", component: { template: "<div>概览内容</div>" } },
        { path: "/:pathMatch(.*)*", component: { template: "<div />" } },
      ],
    });
    await router.push("/");
    await router.isReady();
    const wrapper = mount(AdminLayout, {
      global: { plugins: [[PrimeVue, { ripple: false }], router] },
    });
    await flushPromises();
    return wrapper;
  }

  it("uses the PrimeVue 4 menu in the static desktop layout", async () => {
    const wrapper = await renderLayout();

    expect(wrapper.find(".admin-mobile-bar").exists()).toBe(false);
    expect(wrapper.find(".admin-sidebar .p-menu").exists()).toBe(true);
    expect(wrapper.find(".admin-sidebar-title .pi").exists()).toBe(false);
    expect(wrapper.find(".admin-sidebar-footer").exists()).toBe(true);
    expect(wrapper.find('[aria-label="打开菜单"]').exists()).toBe(false);
    expect(wrapper.text()).toContain("运营管理");
  });

  it("opens the PrimeVue Drawer on mobile", async () => {
    const wrapper = await renderLayout();
    mobile = true;
    changeListener?.({ matches: true } as MediaQueryListEvent);
    await flushPromises();

    const trigger = wrapper.get('[aria-label="打开菜单"]');
    await trigger.trigger("click");
    await flushPromises();
    expect(document.querySelector(".p-drawer")).not.toBeNull();
    wrapper.unmount();
  });
});
