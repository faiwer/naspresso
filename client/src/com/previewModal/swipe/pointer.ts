import { DRAG_SLOP_PX } from "./gesture";

export type Drag = {
  /** Touch this drag follows. Other pointers are ignored. */
  pointerId: number;
  /** `clientX` where the finger went down. Travel is measured from here. */
  startX: number;
  /** `clientX` of the previous sample, used for speed. */
  lastX: number;
  /** `timeStamp` of the previous sample, in milliseconds. */
  lastT: number;
  /** Horizontal speed in pixels per millisecond. Positive moves toward the previous image. */
  velocityX: number;
  /** The finger has moved at least {@link DRAG_SLOP_PX}, so this is a drag. */
  pastSlop: boolean;
};

export const startDrag = (event: PointerEvent): Drag => ({
  pointerId: event.pointerId,
  startX: event.clientX,
  lastX: event.clientX,
  lastT: event.timeStamp,
  velocityX: 0,
  pastSlop: false,
});

/** The latest sample, including travel from the drag's start. */
export const sampleDrag = (
  drag: Drag,
  event: PointerEvent,
): { drag: Drag; dx: number } => {
  const dx = event.clientX - drag.startX;
  const dt = event.timeStamp - drag.lastT;
  return {
    dx,
    drag: {
      ...drag,
      lastX: event.clientX,
      lastT: event.timeStamp,
      velocityX: dt > 0 ? (event.clientX - drag.lastX) / dt : drag.velocityX,
      pastSlop: drag.pastSlop || Math.abs(dx) >= DRAG_SLOP_PX,
    },
  };
};

/** Finger travel and speed at the moment of release. */
export const sampleRelease = (
  drag: Drag,
  event: PointerEvent,
): { dx: number; velocityX: number } => {
  const dt = event.timeStamp - drag.lastT;
  return {
    dx: event.clientX - drag.startX,
    velocityX: dt > 0 ? (event.clientX - drag.lastX) / dt : drag.velocityX,
  };
};
