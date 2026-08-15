import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { tablePreviewView } from "../tablePreviewData";

const mocks = vi.hoisted(() => ({
  createRuntime: vi.fn(),
  destroyRuntime: vi.fn(),
  updateRuntimeTablecloth: vi.fn(() => Promise.resolve()),
  renderTable: vi.fn(),
  setTableDangerTiles: vi.fn(),
  setTableTileHighlight: vi.fn(),
}));

vi.mock("./runtime", () => ({
  createRuntime: mocks.createRuntime,
  destroyRuntime: mocks.destroyRuntime,
  updateRuntimeTablecloth: mocks.updateRuntimeTablecloth,
}));
vi.mock("./scene", () => ({ renderTable: mocks.renderTable }));
vi.mock("./tileHighlight", () => ({
  setTableDangerTiles: mocks.setTableDangerTiles,
  setTableTileHighlight: mocks.setTableTileHighlight,
}));

import { GameTable } from "./GameTable";

describe("GameTable runtime 生命周期", () => {
  let container: HTMLDivElement;
  let root: Root;
  let runtime: Record<string, unknown>;
  let animationFrames: Map<number, FrameRequestCallback>;
  let nextAnimationFrameId: number;

  beforeEach(() => {
    vi.clearAllMocks();
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
    animationFrames = new Map();
    nextAnimationFrameId = 1;
    vi.spyOn(window, "requestAnimationFrame").mockImplementation((callback) => {
      const frameId = nextAnimationFrameId++;
      animationFrames.set(frameId, callback);
      return frameId;
    });
    vi.spyOn(window, "cancelAnimationFrame").mockImplementation((frameId) => {
      animationFrames.delete(frameId);
    });
    runtime = {
      cameraOverride: null,
      tileScale: 1,
      tileWidthRatio: 0.72,
      tableclothPath: "/cloth-a.png",
      resize: vi.fn(),
      rebuild: vi.fn(),
    };
    mocks.createRuntime.mockResolvedValue(runtime);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.restoreAllMocks();
  });

  it("React 视图更新与桌布变化都不会重建 WebGL runtime", async () => {
    await render(1, "/cloth-a.png");
    await render(2, "/cloth-b.png");
    flushAnimationFrames();

    expect(mocks.createRuntime).toHaveBeenCalledTimes(1);
    expect(mocks.destroyRuntime).not.toHaveBeenCalled();
    expect(mocks.renderTable).toHaveBeenLastCalledWith(
      runtime,
      expect.objectContaining({ version: 2 }),
      "play",
      [2, 5],
      [],
      [],
    );
    expect(mocks.updateRuntimeTablecloth).toHaveBeenCalledWith(
      runtime,
      "/cloth-b.png",
    );
  });

  it("同一显示帧内的多次 React 更新只提交最后一个牌桌状态", async () => {
    await render(1, "/cloth-a.png");
    const initialRenderCount = mocks.renderTable.mock.calls.length;

    await render(2, "/cloth-a.png");
    await render(3, "/cloth-a.png");
    flushAnimationFrames();

    expect(mocks.renderTable).toHaveBeenCalledTimes(initialRenderCount + 1);
    expect(mocks.renderTable).toHaveBeenLastCalledWith(
      runtime,
      expect.objectContaining({ version: 3 }),
      "play",
      [2, 5],
      [],
      [],
    );
  });

  it("与牌局状态无关的 React 重渲染不会触发 Three.js 场景提交", async () => {
    await render(1, "/cloth-a.png");
    const initialRenderCount = mocks.renderTable.mock.calls.length;

    await render(1, "/cloth-a.png");
    flushAnimationFrames();

    expect(mocks.createRuntime).toHaveBeenCalledTimes(1);
    expect(mocks.renderTable).toHaveBeenCalledTimes(initialRenderCount);
  });

  async function render(version: number, tableclothPath: string) {
    await act(async () => {
      root.render(
        <GameTable
          view={{ ...tablePreviewView, version }}
          openingPhase="play"
          dice={[2, 5]}
          onTileDiscard={() => {}}
          tableclothPath={tableclothPath}
        />,
      );
      await Promise.resolve();
    });
  }

  function flushAnimationFrames() {
    act(() => {
      const callbacks = [...animationFrames.values()];
      animationFrames.clear();
      callbacks.forEach((callback) => callback(performance.now()));
    });
  }
});
