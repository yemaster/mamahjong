import { afterEach, describe, expect, it } from "vitest";
import {
  clientPointInViewport,
  clientRectInViewport,
  clientVectorInViewport,
} from "./viewportCoordinates";

function rect(
  left: number,
  top: number,
  width: number,
  height: number,
): DOMRect {
  return {
    left,
    top,
    width,
    height,
    right: left + width,
    bottom: top + height,
    x: left,
    y: top,
    toJSON: () => ({}),
  };
}

afterEach(() => {
  document.body.replaceChildren();
});

describe("viewport coordinate conversion", () => {
  it("keeps coordinates upright in an unrotated viewport", () => {
    const viewport = document.createElement("div");
    viewport.className = "auto-rotating-viewport";
    viewport.dataset.rotated = "false";
    viewport.getBoundingClientRect = () => rect(10, 20, 844, 390);
    const element = document.createElement("div");
    element.getBoundingClientRect = () => rect(110, 70, 200, 80);
    viewport.appendChild(element);
    document.body.appendChild(viewport);

    expect(clientPointInViewport(element, 160, 90)).toEqual({ x: 150, y: 70 });
    expect(clientRectInViewport(element)).toEqual({
      left: 100,
      top: 50,
      width: 200,
      height: 80,
    });
    expect(clientVectorInViewport(element, 12, -7)).toEqual({ x: 12, y: -7 });
  });

  it("undoes a clockwise 90 degree viewport rotation", () => {
    const viewport = document.createElement("div");
    viewport.className = "auto-rotating-viewport";
    viewport.dataset.rotated = "true";
    /* 844×390 的逻辑视口旋转后，占据 390×844 的物理屏幕。 */
    viewport.getBoundingClientRect = () => rect(0, 0, 390, 844);
    const element = document.createElement("div");
    /* logical(left=100, top=50, width=200, height=80) 旋转后的外接矩形。 */
    element.getBoundingClientRect = () => rect(260, 100, 80, 200);
    viewport.appendChild(element);
    document.body.appendChild(viewport);

    expect(clientPointInViewport(element, 320, 150)).toEqual({ x: 150, y: 70 });
    expect(clientRectInViewport(element)).toEqual({
      left: 100,
      top: 50,
      width: 200,
      height: 80,
    });
    expect(clientVectorInViewport(element, -20, 10)).toEqual({ x: 10, y: 20 });
  });
});
