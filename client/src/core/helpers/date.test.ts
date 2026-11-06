import { describe, expect, it } from "vitest";

import { formatLockRemaining } from "./date";

describe("formatLockRemaining", () => {
  it("uses minutes until the last minute", () => {
    expect(formatLockRemaining(15 * 60 * 1000)).toBe("15 min");
    expect(formatLockRemaining(60 * 1000)).toBe("1 min");
    expect(formatLockRemaining(61 * 1000)).toBe("1 min");
  });

  it("uses seconds under a minute", () => {
    expect(formatLockRemaining(59 * 1000)).toBe("59 sec");
    expect(formatLockRemaining(0)).toBe("0 sec");
    expect(formatLockRemaining(-1000)).toBe("0 sec");
  });
});
