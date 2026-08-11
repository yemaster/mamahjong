// @vitest-environment jsdom

import { mount } from "@vue/test-utils";
import PrimeVue from "primevue/config";
import { describe, expect, it } from "vitest";
import PageShell from "./PageShell.vue";

describe("PageShell", () => {
  it("uses an opaque skeleton panel while content is loading", () => {
    const wrapper = mount(PageShell, {
      props: { title: "角色", loading: true },
      slots: { default: "<div data-content>列表内容</div>" },
      global: { plugins: [[PrimeVue, { ripple: false }]] },
    });

    expect(wrapper.find(".page-loading-panel").exists()).toBe(true);
    expect(wrapper.find("[data-content]").exists()).toBe(false);
  });
});
