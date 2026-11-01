import { describe, expect, it } from "vitest";

import { isFrameEnabled } from "./frameEnabled";

const none = { center: false, right: false };

describe("isFrameEnabled", () => {
  it("always enables the center", () => {
    expect(isFrameEnabled("center", null, none, true)).toBe(true);
  });

  it("enables the right frame once the center image has downloaded", () => {
    expect(isFrameEnabled("right", null, none, true)).toBe(false);
    expect(
      isFrameEnabled("right", null, { center: true, right: false }, true),
    ).toBe(true);
  });

  it("enables the left frame after the center and the right image", () => {
    expect(
      isFrameEnabled("left", null, { center: true, right: false }, true),
    ).toBe(false);
    expect(
      isFrameEnabled("left", null, { center: true, right: true }, true),
    ).toBe(true);
  });

  it("does not wait for a right image that is not there", () => {
    expect(
      isFrameEnabled("left", null, { center: true, right: false }, false),
    ).toBe(true);
  });

  it("enables a side as soon as it is promoted to center", () => {
    expect(isFrameEnabled("left", "left", none, true)).toBe(true);
    expect(isFrameEnabled("right", "right", none, true)).toBe(true);
  });
});
