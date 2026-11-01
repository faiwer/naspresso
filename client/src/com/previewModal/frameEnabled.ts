export type FrameSide = "left" | "center" | "right";

/**
 * Center first, then the right neighbor, then the left. `promoted` is the
 * neighbor sliding into the center, so it loads immediately; callers pass
 * `null` until that swipe exists.
 */
export const isFrameEnabled = (
  side: FrameSide,
  promoted: FrameSide | null,
  downloaded: { center: boolean; right: boolean },
  hasRight: boolean,
): boolean => {
  if (side === "center" || side === promoted) {
    return true;
  }
  if (side === "right") {
    return downloaded.center;
  }
  return downloaded.center && (!hasRight || downloaded.right);
};
