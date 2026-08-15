import { afterEach, describe, expect, it, vi } from "vitest";
import {
  SCENE_TRANSITION_MIST,
  preloadSceneTransitionMist,
} from "./sceneTransitionAssets";

describe("scene transition assets", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("登录页只下载一次雾图并等待图片解码", async () => {
    const images: MockImage[] = [];

    class MockImage {
      onload: (() => void) | null = null;
      onerror: (() => void) | null = null;
      complete = false;
      naturalWidth = 0;
      decoding = "auto";
      source = "";
      decode = vi.fn(() => Promise.resolve());

      constructor() {
        images.push(this);
      }

      set src(value: string) {
        this.source = value;
      }
    }

    vi.stubGlobal("Image", MockImage);
    const firstLoad = preloadSceneTransitionMist();
    const secondLoad = preloadSceneTransitionMist();

    expect(secondLoad).toBe(firstLoad);
    expect(images).toHaveLength(1);
    expect(images[0]?.source).toBe(SCENE_TRANSITION_MIST);
    expect(images[0]?.decoding).toBe("async");

    images[0]?.onload?.();
    await firstLoad;
    expect(images[0]?.decode).toHaveBeenCalledOnce();
  });
});
