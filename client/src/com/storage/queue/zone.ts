import type { Zone } from "./types";

/** `rootBounds` includes the margin, so the scrollport's own box splits the band. */
export const zoneOf = (
  entry: IntersectionObserverEntry,
  port: DOMRect,
): Zone => {
  if (!entry.isIntersecting) {
    return "out";
  }

  const rect = entry.boundingClientRect;
  if (rect.bottom <= port.top) {
    return "above";
  }

  if (rect.top >= port.bottom) {
    return "below";
  }

  return "visible";
};

/** Waiting jobs start on screen, then below, then above. `out` is not queued. */
export const zoneRank: Record<Zone, number> = {
  visible: 0,
  below: 1,
  above: 2,
  out: 2,
};
