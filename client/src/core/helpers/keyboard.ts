/**
 * The shortcut `event` is. Modifiers come first, in the order `ctrl`, `alt`,
 * `cmd`, `shift`, then the key. A letter is uppercase: Ctrl+A is `ctrl-A`,
 * Cmd+Shift+A is `cmd-shift-A`. Unmodified Escape is `Escape`.
 */
export const formatKeyChord = (event: KeyboardEvent): string => {
  const parts: string[] = [];
  if (event.ctrlKey) {
    parts.push("ctrl");
  }
  if (event.altKey) {
    parts.push("alt");
  }
  if (event.metaKey) {
    parts.push("cmd");
  }
  if (event.shiftKey) {
    parts.push("shift");
  }
  if (!MODIFIER_KEYS.has(event.key)) {
    parts.push(keyName(event.key));
  }
  return parts.join("-");
};

/** Keys that are already represented by a modifier prefix. */
const MODIFIER_KEYS = new Set(["Control", "Shift", "Alt", "Meta", "AltGraph"]);

const keyName = (key: string): string => {
  if (key === " ") {
    return "Space";
  }
  return key.length === 1 ? key.toUpperCase() : key;
};
