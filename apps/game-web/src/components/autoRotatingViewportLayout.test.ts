import { describe, expect, it } from "vitest";
import { autoRotatingViewportFrame } from "./autoRotatingViewportLayout";

describe("autoRotatingViewportFrame", () => {
  it.each([
    [1600, 900, false, 1600, 900, 1],
    [844, 390, false, 844, 390, 0.43333333333333335],
    [390, 844, true, 844, 390, 0.43333333333333335],
    [1024, 768, false, 1024, 768, 0.64],
    [768, 1024, true, 1024, 768, 0.64],
  ])(
    "chooses the larger fit for %sx%s",
    (width, height, rotated, logicalWidth, logicalHeight, scale) => {
      const frame = autoRotatingViewportFrame(width, height);
      expect(frame.rotated).toBe(rotated);
      expect(frame.width).toBe(logicalWidth);
      expect(frame.height).toBe(logicalHeight);
      expect(frame.scale).toBeCloseTo(scale);
    },
  );

  it("keeps the current direction when both fits are equal", () => {
    expect(autoRotatingViewportFrame(900, 900).rotated).toBe(false);
  });

  it("falls back to the design viewport before a measurable size exists", () => {
    expect(autoRotatingViewportFrame(0, 0)).toEqual({
      width: 1600,
      height: 900,
      rotated: false,
      scale: 1,
    });
  });
});
