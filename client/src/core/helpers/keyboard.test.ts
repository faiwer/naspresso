import { describe, expect, it } from "vitest";

import { formatKeyChord } from "./keyboard";

const key = (init: KeyboardEventInit): KeyboardEvent =>
  new KeyboardEvent("keydown", init);

describe("formatKeyChord", () => {
  it("names an unmodified key as itself", () => {
    expect(formatKeyChord(key({ key: "Escape" }))).toBe("Escape");
    expect(formatKeyChord(key({ key: "ArrowLeft" }))).toBe("ArrowLeft");
  });

  it("puts modifiers in a fixed order and uppercases a letter", () => {
    expect(formatKeyChord(key({ key: "a", ctrlKey: true }))).toBe("ctrl-A");
    expect(
      formatKeyChord(key({ key: "A", metaKey: true, shiftKey: true })),
    ).toBe("cmd-shift-A");
    expect(
      formatKeyChord(
        key({
          key: "a",
          ctrlKey: true,
          altKey: true,
          metaKey: true,
          shiftKey: true,
        }),
      ),
    ).toBe("ctrl-alt-cmd-shift-A");
  });

  it("keeps a modified Escape distinct from Escape", () => {
    expect(formatKeyChord(key({ key: "Escape", ctrlKey: true }))).toBe(
      "ctrl-Escape",
    );
  });

  it("does not repeat a modifier key that is the event itself", () => {
    expect(formatKeyChord(key({ key: "Control", ctrlKey: true }))).toBe("ctrl");
  });

  it("names the space bar", () => {
    expect(formatKeyChord(key({ key: " ", ctrlKey: true }))).toBe("ctrl-Space");
  });
});
