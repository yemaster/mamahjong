import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { SplashScreen } from "./SplashScreen";

describe("SplashScreen", () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.useFakeTimers();
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.useRealTimers();
  });

  it("loads the background before preparing the lobby and entering", async () => {
    const onEnter = vi.fn();
    const onLogout = vi.fn();
    const prepareGame = vi.fn(
      async (reportProgress: (progress: number) => void) => {
        reportProgress(34);
        await Promise.resolve();
        reportProgress(100);
      },
    );

    act(() =>
      root.render(
        <SplashScreen
          onEnter={onEnter}
          onLogout={onLogout}
          prepareGame={prepareGame}
        />,
      ),
    );

    const screen =
      container.querySelector<HTMLButtonElement>(".splash-entry-hit-area");
    const splash = container.querySelector(".splash-screen");
    const background =
      container.querySelector<HTMLImageElement>(".splash-background");
    const developer =
      container.querySelector<HTMLImageElement>(".splash-developer-logo img");
    expect(screen?.disabled).toBe(true);
    expect(container.textContent).not.toContain("欢迎您");
    expect(splash?.classList.contains("has-background")).toBe(false);
    expect(developer).not.toBeNull();
    expect(container.querySelector(".splash-logo__image")).not.toBeNull();
    expect(container.querySelectorAll(".sakura-petal")).toHaveLength(28);
    expect(container.querySelector(".splash-loader")).not.toBeNull();
    expect(prepareGame).not.toHaveBeenCalled();

    act(() => {
      background?.dispatchEvent(new Event("load"));
      developer?.dispatchEvent(new Event("load"));
    });
    expect(splash?.classList.contains("has-background")).toBe(false);
    expect(prepareGame).not.toHaveBeenCalled();

    act(() => vi.advanceTimersByTime(1_699));
    expect(splash?.classList.contains("is-game-logo-visible")).toBe(false);
    expect(splash?.classList.contains("has-background")).toBe(false);

    act(() => vi.advanceTimersByTime(1));
    expect(splash?.classList.contains("is-developer-logo-fading")).toBe(true);
    expect(splash?.classList.contains("is-game-logo-visible")).toBe(false);
    expect(splash?.classList.contains("has-background")).toBe(false);

    act(() => vi.advanceTimersByTime(799));
    expect(splash?.classList.contains("is-game-logo-visible")).toBe(false);
    expect(splash?.classList.contains("has-background")).toBe(false);

    act(() => vi.advanceTimersByTime(1));
    expect(splash?.classList.contains("is-game-logo-visible")).toBe(true);
    expect(splash?.classList.contains("has-background")).toBe(false);

    act(() => vi.advanceTimersByTime(2_299));
    expect(splash?.classList.contains("has-background")).toBe(false);

    act(() => vi.advanceTimersByTime(1));
    expect(splash?.classList.contains("has-background")).toBe(true);

    await act(async () => {
      vi.advanceTimersByTime(799);
      await Promise.resolve();
    });
    expect(prepareGame).not.toHaveBeenCalled();

    await act(async () => {
      vi.advanceTimersByTime(1);
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(prepareGame).toHaveBeenCalledOnce();
    expect(
      container.querySelector(".splash-loader")?.classList.contains("is-visible"),
    ).toBe(false);

    act(() =>
      root.render(
        <SplashScreen
          onEnter={onEnter}
          onLogout={onLogout}
          prepareGame={prepareGame}
          welcomeName="雀士"
        />,
      ),
    );

    expect(container.textContent).toContain("欢迎您，雀士。");
    expect(
      container.querySelector(".splash-loader")?.classList.contains("is-visible"),
    ).toBe(true);
    expect(container.querySelector(".splash-enter")?.classList.contains("is-visible")).toBe(false);
    expect(container.textContent).not.toContain("加载中");

    act(() => vi.advanceTimersByTime(419));
    expect(screen?.disabled).toBe(true);

    act(() => vi.advanceTimersByTime(1));
    expect(screen?.disabled).toBe(false);
    expect(container.querySelector(".splash-loader")?.classList.contains("is-visible")).toBe(false);
    expect(container.querySelector(".splash-enter")?.classList.contains("is-visible")).toBe(true);
    expect(container.textContent).toContain("点击进入游戏");
    expect(container.querySelector('[aria-label="进入全屏"]')).not.toBeNull();
    expect(container.querySelector('[aria-label="退出登录"]')).not.toBeNull();

    act(() => screen?.click());
    expect(onEnter).toHaveBeenCalledOnce();

    act(() =>
      container.querySelector<HTMLButtonElement>('[aria-label="退出登录"]')?.click(),
    );
    expect(onLogout).toHaveBeenCalledOnce();
    expect(container.querySelector(".splash-actions")?.classList.contains("is-visible")).toBe(false);
  });
});
