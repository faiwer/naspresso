/** An unmodified primary-button click. */
export const isPlainClick = (event: MouseEvent): boolean =>
  event.button === 0 &&
  !event.metaKey &&
  !event.ctrlKey &&
  !event.shiftKey &&
  !event.altKey;

/** Chromium sets this on the mouse events a long-press imitates. */
const firesTouchEvents = (event: Event): boolean =>
  "sourceCapabilities" in event &&
  (event as { sourceCapabilities?: { firesTouchEvents?: boolean } })
    .sourceCapabilities?.firesTouchEvents === true;

/**
 * A long-press is reported as a mouse event. `firesTouchEvents` is how
 * Chromium marks that imitation.
 */
export const isTrueMouseEvent = (event: PointerEvent): boolean =>
  event.pointerType === "mouse" && !firesTouchEvents(event);
