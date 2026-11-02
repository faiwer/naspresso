import { describe, expect, it } from "vitest";

import {
  commitTranslate,
  getCommitDelta,
  dragTranslate,
  FLICK_MS,
  neighborShift,
  rubberBand,
  SNAP_FRACTION,
  getSnapDistance,
} from "./gesture";

describe("snapDistance", () => {
  it("is the viewport width when the current image fills it", () => {
    expect(getSnapDistance(1280, 500)).toBe(890);
    expect(getSnapDistance(1280, 1280)).toBe(1280);
  });
});

describe("rubberBand", () => {
  it("stays below the viewport size", () => {
    expect(rubberBand(10_000, 1280)).toBeLessThan(1280);
    expect(rubberBand(0, 1280)).toBe(0);
  });
});

describe("dragTranslate", () => {
  it("follows the finger until the current image is fully off screen", () => {
    expect(dragTranslate(-400, 1280, 1280, true)).toBe(-400);
  });

  it("resists past the snap distance", () => {
    const snap = getSnapDistance(1280, 1280);
    const translate = dragTranslate(-(snap + 200), 1280, 1280, true);
    expect(translate).toBeLessThan(-snap);
    expect(translate).toBeGreaterThan(-(snap + 200));
  });

  it("resists the whole drag when there is no neighbor", () => {
    expect(dragTranslate(-200, 1280, 1280, false)).toBeCloseTo(
      -rubberBand(200, 1280),
    );
  });
});

describe("neighborShift", () => {
  it("lands a narrower next image centered as the current one leaves", () => {
    const viewport = 1561;
    const current = 924;
    const next = 740;
    const snap = getSnapDistance(viewport, current);
    const translate = -snap;
    const shift = neighborShift(translate, snap, current, next, "right");
    const currentRight = (viewport + current) / 2 + translate;
    const nextLeft = viewport + shift + translate;
    expect(currentRight).toBeCloseTo(0);
    expect(nextLeft).toBeCloseTo((viewport - next) / 2);
  });

  it("stays flush while the track has not moved", () => {
    expect(neighborShift(0, 1000, 924, 740, "right")).toBe(0);
    expect(neighborShift(0, 1000, 924, 740, "left")).toBe(0);
  });
});

describe("commitTranslate", () => {
  it("moves the track so the chosen neighbor lands in the center", () => {
    expect(commitTranslate(1, 890)).toBe(-890);
    expect(commitTranslate(-1, 890)).toBe(890);
  });
});

describe("commitDelta", () => {
  const snap = getSnapDistance(1280, 1280);
  const line = snap * SNAP_FRACTION;

  it("commits a slow finger that crossed the line", () => {
    expect(getCommitDelta(-(line + 1), 0, snap)).toBe(1);
    expect(getCommitDelta(line + 1, 0, snap)).toBe(-1);
  });

  it("rolls back a slow finger that stopped short", () => {
    expect(getCommitDelta(-(line - 1), 0, snap)).toBe(0);
  });

  it("commits a flick that stopped short of the line", () => {
    const dx = -40;
    const velocityX = -((line - dx) / FLICK_MS);
    expect(getCommitDelta(dx, velocityX, snap)).toBe(1);
  });

  it("ignores speed toward the starting photo", () => {
    expect(getCommitDelta(-40, 5, snap)).toBe(0);
  });

  it("rolls back at the end of the list", () => {
    expect(getCommitDelta(-400, -5, null)).toBe(0);
  });
});
