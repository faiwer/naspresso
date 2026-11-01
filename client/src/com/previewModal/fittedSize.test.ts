import { describe, expect, it } from "vitest";

import { fittedSize } from "./fittedSize";

describe("fittedSize", () => {
  it("keeps an image that already fits", () => {
    expect(
      fittedSize({ width: 400, height: 300 }, { width: 800, height: 600 }),
    ).toEqual({ width: 400, height: 300 });
  });

  it("shrinks a wide image to the viewport width", () => {
    expect(
      fittedSize({ width: 2000, height: 1000 }, { width: 800, height: 600 }),
    ).toEqual({ width: 800, height: 400 });
  });

  it("shrinks a vertical image to the viewport height", () => {
    expect(
      fittedSize({ width: 1000, height: 2000 }, { width: 800, height: 600 }),
    ).toEqual({ width: 300, height: 600 });
  });
});
