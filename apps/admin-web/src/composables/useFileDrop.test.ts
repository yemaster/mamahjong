import { describe, expect, it, vi } from "vitest";
import { acceptsAssetFile, useFileDrop } from "./useFileDrop";

function dragEvent(files: File[] = []): DragEvent {
  return {
    dataTransfer: {
      types: ["Files"],
      files,
      dropEffect: "none",
    },
  } as unknown as DragEvent;
}

describe("file drop", () => {
  it("tracks nested drag events and delivers dropped files", () => {
    const receive = vi.fn();
    const drop = useFileDrop(receive);
    const event = dragEvent([new File(["image"], "hero.png", { type: "image/png" })]);

    drop.enter(event);
    drop.enter(event);
    drop.leave();
    expect(drop.active.value).toBe(true);

    drop.drop(event);
    expect(drop.active.value).toBe(false);
    expect(receive).toHaveBeenCalledWith([expect.objectContaining({ name: "hero.png" })]);
  });

  it("checks MIME types and falls back to file extensions", () => {
    expect(acceptsAssetFile(new File([], "hero.png"), "image")).toBe(true);
    expect(acceptsAssetFile(new File([], "theme.ogg"), "audio")).toBe(true);
    expect(acceptsAssetFile(new File([], "notes.txt"), "image")).toBe(false);
  });
});
