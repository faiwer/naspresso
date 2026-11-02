import { useRef } from "react";

import type { StorageFile } from "../../../api/storage.io";
import {
  getCommitDelta,
  dragTranslate,
  getSnapDistance,
  commitTranslate,
} from "./gesture";
import { type Motion } from "./motion";
import { sampleDrag, sampleRelease, startDrag, type Drag } from "./pointer";
import { frameWidth, neighborAt, type Viewport } from "./widths";

type UseDragGestureArgs = {
  files: StorageFile[];
  pos: number;
  viewport: Viewport | null;
  settling: boolean;
  setMotion: (update: (motion: Motion) => Motion) => void;
};

/** Pointer handlers for one touch drag across the strip. */
export const useDragGesture = ({
  files,
  pos,
  viewport,
  settling,
  setMotion,
}: UseDragGestureArgs) => {
  const dragRef = useRef<Drag | null>(null);

  const onPointerDown = (event: PointerEvent) => {
    if (
      // No mouse-swipe gesture.
      event.pointerType !== "touch" ||
      // No swipes during the release animation.
      settling ||
      // A second finger?
      dragRef.current
    ) {
      return;
    }
    dragRef.current = startDrag(event);
  };

  const onPointerMove = (event: PointerEvent) => {
    const drag = dragRef.current;
    if (
      // Dragging is not in progress.
      !drag ||
      // Wrong finger
      event.pointerId !== drag.pointerId ||
      // The viewport dimensions are not yet available. Impossible
      viewport == null
    ) {
      return;
    }

    const sample = sampleDrag(drag, event);
    dragRef.current = sample.drag;

    if (!sample.drag.pastSlop) {
      return;
    }

    const currentWidth = frameWidth(files[pos], viewport);
    const neighbor = neighborAt(files, pos, sample.dx, viewport);
    setMotion((prev) => ({
      ...prev,
      translateX: dragTranslate(
        sample.dx,
        viewport.width,
        currentWidth,
        neighbor != null,
      ),
      promoted: neighbor?.side ?? null,
    }));
  };

  const onPointerUp = (event: PointerEvent) => {
    const drag = dragRef.current;
    if (
      // Dragging is not in progress.
      !drag ||
      // Wrong finger
      event.pointerId !== drag.pointerId
    ) {
      return;
    }

    dragRef.current = null; // Stop dragging.

    if (!drag.pastSlop || viewport == null) {
      return; // The movement was too short to be a real drag.
    }

    const { dx, velocityX } = sampleRelease(drag, event);
    const currentWidth = frameWidth(files[pos], viewport);
    const neighbor = neighborAt(files, pos, dx, viewport);
    const snapDistance = neighbor
      ? getSnapDistance(viewport.width, currentWidth)
      : null;
    const delta = getCommitDelta(dx, velocityX, snapDistance);
    const translateX =
      delta === 0 || snapDistance == null
        ? 0
        : commitTranslate(delta, snapDistance);

    setMotion((prev) => ({
      ...prev,
      translateX,
      settling: true,
      delta,
    }));
  };

  return {
    onPointerDown,
    onPointerMove,
    onPointerUp,
    onPointerCancel: onPointerUp,
  };
};
