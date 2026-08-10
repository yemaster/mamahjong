import { act, useEffect, useState } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  SceneModuleLoaded,
  SceneTransition,
  useSceneReady,
  useSceneWaitingPeers,
} from "./SceneTransition";

/** 每 100ms 多一个人load完，四个人齐了这一幕才算好。 */
function PeerWaitScene() {
  const [ready, setReady] = useState(1);
  useSceneWaitingPeers(ready < 4 ? { ready, total: 4 } : null);
  useSceneReady(ready >= 4);

  useEffect(() => {
    const timer = setInterval(() => setReady((current) => current + 1), 100);
    return () => clearInterval(timer);
  }, []);

  return <div>牌局</div>;
}

function DeferredScene() {
  const [ready, setReady] = useState(false);
  useSceneReady(ready);

  useEffect(() => {
    const timer = setTimeout(() => setReady(true), 300);
    return () => clearTimeout(timer);
  }, []);

  return <div>房间</div>;
}

describe("SceneTransition", () => {
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

  it("gathers first, waits for real readiness, then always reveals fully", () => {
    act(() => {
      root.render(
        <SceneTransition sceneKey="lobby">
          <div>雀庄</div>
        </SceneTransition>,
      );
    });

    act(() => {
      root.render(
        <SceneTransition sceneKey="room:1">
          <SceneModuleLoaded>
            <DeferredScene />
          </SceneModuleLoaded>
        </SceneTransition>,
      );
    });

    expect(container.textContent).toContain("雀庄");
    expect(
      container.querySelector(".scene-transition--gathering"),
    ).not.toBeNull();

    act(() => vi.advanceTimersByTime(649));
    expect(container.textContent).not.toContain("房间");
    expect(
      container.querySelector(".scene-transition--gathering"),
    ).not.toBeNull();

    act(() => vi.advanceTimersByTime(1));
    expect(container.textContent).toContain("房间");
    expect(container.querySelector(".scene-transition--loading")).not.toBeNull();

    act(() => vi.advanceTimersByTime(299));
    expect(container.querySelector(".scene-transition--loading")).not.toBeNull();

    act(() => vi.advanceTimersByTime(1));
    expect(
      container.querySelector<HTMLElement>(".scene-transition__loader-fill")
        ?.style.width,
    ).toBe("100%");
    expect(container.querySelector(".scene-transition--loading")).not.toBeNull();

    act(() => vi.advanceTimersByTime(420));
    expect(
      container.querySelector(".scene-transition--revealing"),
    ).not.toBeNull();

    act(() => vi.advanceTimersByTime(699));
    expect(
      container.querySelector(".scene-transition--revealing"),
    ).not.toBeNull();

    act(() => vi.advanceTimersByTime(1));
    expect(container.querySelector(".scene-transition--idle")).not.toBeNull();
  });

  it("等人的时候不许再退回加载图", () => {
    act(() => {
      root.render(
        <SceneTransition sceneKey="lobby">
          <div>雀庄</div>
        </SceneTransition>,
      );
    });

    act(() => {
      root.render(
        <SceneTransition sceneKey="match:1">
          <SceneModuleLoaded>
            <PeerWaitScene />
          </SceneModuleLoaded>
        </SceneTransition>,
      );
    });

    act(() => vi.advanceTimersByTime(650));
    expect(container.textContent).toContain("等待其他玩家(1/4)");
    expect(container.querySelector(".scene-transition__loader")).toBeNull();

    act(() => vi.advanceTimersByTime(200));
    expect(container.textContent).toContain("等待其他玩家(3/4)");

    /* 人齐了，云雾还得散一秒多，这段时间不能闪回加载图。 */
    act(() => vi.advanceTimersByTime(100));
    expect(container.querySelector(".scene-transition__loader")).toBeNull();
    expect(container.textContent).toContain("等待其他玩家");

    act(() => vi.advanceTimersByTime(420));
    expect(
      container.querySelector(".scene-transition--revealing"),
    ).not.toBeNull();
    expect(container.querySelector(".scene-transition__loader")).toBeNull();

    act(() => vi.advanceTimersByTime(700));
    expect(container.querySelector(".scene-transition--idle")).not.toBeNull();
    expect(container.textContent).toContain("牌局");
  });
});
