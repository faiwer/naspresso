/** Finger travel that counts as a drag rather than a tap, in pixels. */
export const DRAG_SLOP_PX = 8;

/** A slow finger commits after this fraction of the way to the neighbor. */
export const SNAP_FRACTION = 0.1;

/** A flick is projected forward by this many milliseconds. */
export const FLICK_MS = 150;

/** How long the settle animation takes, in milliseconds. */
export const SETTLE_MS = 250;

/** Rubber-band resistance. The first pixels move by `1 / this`. */
export const RUBBER_STIFFNESS = 3;

/**
 * How far the track travels to take the current image fully off screen.
 * The neighbor is shifted so this same distance leaves it centered.
 */
export const getSnapDistance = (
  viewportWidth: number,
  currentWidth: number,
): number => (viewportWidth + currentWidth) / 2;

/**
 * Extra x offset that centers a neighbor whose width differs from the current
 * image. Zero until the track moves, so that neighbor's inner edge stays on the
 * viewport edge.
 *
 * snapDistance is [viewportWidth + currentImage.width] / 2. Basically it means
 * that the image has to be fully off screen to be considered as settled. But
 * due to the difference in width, the neighbour image wouldn't be properly
 * centered at that moment.
 */
export const neighborShift = (
  translateX: number,
  snapDistance: number,
  currentWidth: number,
  neighborWidth: number,
  side: "left" | "right",
): number => {
  if (snapDistance <= 0) {
    return 0; // No drag or settle animation.
  }

  const progress =
    side === "right"
      ? Math.max(0, -translateX / snapDistance)
      : Math.max(0, translateX / snapDistance);
  if (progress === 0) {
    return 0;
  }
  const outward = ((currentWidth - neighborWidth) / 2) * progress;
  return side === "right" ? outward : -outward;
};

/**
 * Offset while there is nothing to land on. It approaches `size` and never
 * reaches it.
 */
export const rubberBand = (distance: number, size: number): number => {
  if (distance <= 0 || size <= 0) {
    return 0;
  }
  return (distance * size) / (distance + RUBBER_STIFFNESS * size);
};

/**
 * Track `translateX` while the finger is down. Positive `dx` moves the
 * picture right.
 */
export const dragTranslate = (
  dx: number,
  viewportWidth: number,
  currentWidth: number,
  hasNeighbor: boolean,
): number => {
  const sign = Math.sign(dx);
  if (sign === 0) {
    return 0;
  }

  const distance = Math.abs(dx);

  if (!hasNeighbor) {
    return sign * rubberBand(distance, viewportWidth);
  }

  const snap = getSnapDistance(viewportWidth, currentWidth);
  if (distance <= snap) {
    // Keep the shift in proportion to the distance.
    return dx;
  }

  // Rubber-band the rest.
  return sign * (snap + rubberBand(distance - snap, viewportWidth));
};

/**
 * Index change on release. `-1` is the previous image, `1` the next, `0`
 * slides back. `snap` is null when that side has no image.
 */
export const getCommitDelta = (
  dx: number,
  velocityX: number,
  snapDistance: number | null,
): -1 | 0 | 1 => {
  if (
    // No neighbour
    snapDistance == null ||
    // No movement
    dx === 0
  ) {
    return 0;
  }

  const sign = Math.sign(dx);
  const velocityAbs =
    Math.sign(velocityX) === sign
      ? Math.abs(velocityX)
      : // The shift is big enough to commit, but the latest movement was in the
        // opposite direction. Ignore the speed.
        0;
  const score = Math.abs(dx) + velocityAbs * FLICK_MS;
  if (score < snapDistance * SNAP_FRACTION) {
    // Either too short movement or too slow speed
    return 0;
  }

  return sign > 0 ? -1 : 1;
};

/**
 * Track offset that carries the current image fully off screen toward `delta`.
 * Positive `delta` is the next image, so the track moves left.
 */
export const commitTranslate = (delta: -1 | 1, snapDistance: number): number =>
  delta > 0 ? -snapDistance : snapDistance;

export type SideDownloaded = { left: boolean; center: boolean; right: boolean };
