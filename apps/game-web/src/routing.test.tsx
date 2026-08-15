import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { navigateTo, useGameScene } from "./routing";

function SceneProbe() {
  const scene = useGameScene();
  return (
    <div>
      {scene.kind}
      {scene.kind === "profile"
        ? `:${scene.tab ?? "info"}:${scene.returnRoomId ?? ""}`
        : ""}
    </div>
  );
}

describe("useGameScene", () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    window.history.replaceState(null, "", "#lobby");
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  it("returns a stable snapshot and updates after navigation", () => {
    act(() => root.render(<SceneProbe />));
    expect(container.textContent).toBe("lobby");

    act(() => {
      navigateTo({ kind: "profile" });
      window.dispatchEvent(new HashChangeEvent("hashchange"));
    });

    expect(container.textContent).toBe("profile:info:");
  });

  it("保留角色设置标签和返回房间信息", () => {
    act(() => root.render(<SceneProbe />));
    act(() => {
      navigateTo({
        kind: "profile",
        userId: "user-1",
        tab: "character",
        returnRoomId: "123456",
      });
      window.dispatchEvent(new HashChangeEvent("hashchange"));
    });

    expect(window.location.hash).toContain("tab=character");
    expect(container.textContent).toBe("profile:character:123456");
  });

  it("保留个性化和选项标签", () => {
    act(() => root.render(<SceneProbe />));
    act(() => {
      navigateTo({ kind: "profile", tab: "personalization" });
      window.dispatchEvent(new HashChangeEvent("hashchange"));
    });

    expect(window.location.hash).toContain("tab=personalization");
    expect(container.textContent).toBe("profile:personalization:");

    act(() => {
      navigateTo({ kind: "profile", tab: "options" });
      window.dispatchEvent(new HashChangeEvent("hashchange"));
    });
    expect(window.location.hash).toContain("tab=options");
    expect(container.textContent).toBe("profile:options:");
  });

  it("旧界面和音乐链接统一兼容到个性化", () => {
    window.history.replaceState(null, "", "#profile?tab=music");
    act(() => root.render(<SceneProbe />));
    expect(container.textContent).toBe("profile:personalization:");
  });
});
