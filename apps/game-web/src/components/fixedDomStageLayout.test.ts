import { describe, expect, it } from "vitest";
import {
  fixedDomStageFrame,
  fixedDomStageScale,
  visualPixelsToStage,
} from "./fixedDomStageLayout";

describe("fixedDomStageScale", () => {
  it.each([
    [1600, 900, 1],
    [1920, 1080, 1.2],
    [1024, 768, 0.64],
    [768, 1024, 0.48],
    [390, 844, 0.24375],
    [844, 390, 0.43333333333333335],
  ])("fits %sx%s with one scale", (width, height, expected) => {
    expect(fixedDomStageScale(width, height)).toBeCloseTo(expected);
  });

  it.each([
    [1920, 1080, 0, 0, 1920, 1080],
    [1024, 768, 0, 96, 1024, 576],
    [768, 1024, 0, 296, 768, 432],
    [844, 390, 75.33333333333333, 0, 693.3333333333334, 390],
  ])(
    "centers the stage in %sx%s",
    (viewportWidth, viewportHeight, left, top, width, height) => {
      const frame = fixedDomStageFrame(viewportWidth, viewportHeight);
      expect(frame.left).toBeCloseTo(left);
      expect(frame.top).toBeCloseTo(top);
      expect(frame.width).toBeCloseTo(width);
      expect(frame.height).toBeCloseTo(height);
      expect(frame.left * 2 + frame.width).toBeCloseTo(viewportWidth);
      expect(frame.top * 2 + frame.height).toBeCloseTo(viewportHeight);
    },
  );
});

describe("visualPixelsToStage", () => {
  it.each([
    [48, 0.48, 100],
    [24.375, 0.24375, 100],
    [43.333333333333336, 0.43333333333333335, 100],
    [100, 1, 100],
  ])("maps %s visual pixels at %sx", (pixels, scale, expected) => {
    expect(visualPixelsToStage(pixels, scale)).toBeCloseTo(expected);
  });
});
