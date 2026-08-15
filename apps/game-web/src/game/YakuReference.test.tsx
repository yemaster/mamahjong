import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { projectAboutTemplate } from "./projectAboutData";
import { YakuReferencePage } from "./YakuReference";

describe("YakuReferencePage", () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  it("从帮助主页进入关于模板，并可返回帮助主页", () => {
    act(() => root.render(<YakuReferencePage onBack={() => {}} />));

    const aboutButton = [...container.querySelectorAll("button")].find(
      (button) => button.textContent?.includes("关于本项目"),
    );
    expect(aboutButton).toBeDefined();

    act(() => aboutButton?.click());
    expect(container.querySelector(".project-about")).not.toBeNull();
    expect(container.textContent).toContain(
      projectAboutTemplate.sections[0]!.title,
    );
    expect(container.textContent).toContain("更新日志");
    expect(
      container.querySelectorAll(".project-about__changelog > li"),
    ).toHaveLength(projectAboutTemplate.changelog.length);

    const backButton = container.querySelector<HTMLButtonElement>(
      'button[aria-label="返回帮助主页"]',
    );
    act(() => backButton?.click());
    expect(container.querySelector(".project-about")).toBeNull();
    expect(container.textContent).toContain("立直麻将");
    expect(container.textContent).toContain("冲击麻将");
  });
});
