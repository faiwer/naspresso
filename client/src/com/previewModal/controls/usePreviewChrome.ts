import { useRef, useState } from "react";

import { isTrueMouseEvent } from "../../../core/helpers/mouse";
import { clearRefTimeout } from "../../../core/helpers/timeout";
import { useAsyncEffect } from "../../../core/hooks/useAsyncEffect";

/**
 * Shows the preview chrome while the mouse is moving over `element`, then
 * hides it after {@link IDLE_MS}. Hovering the chrome holds it up.
 */
export const usePreviewChrome = (viewport: HTMLElement) => {
  const [visible, setVisible] = useState(false);
  const timerRef = useRef<number | null>(null);

  const reveal = () => {
    setVisible(true);
    clearRefTimeout(timerRef);
    timerRef.current = window.setTimeout(() => {
      timerRef.current = null;
      setVisible(false);
    }, IDLE_MS);
  };

  const hold = () => {
    setVisible(true);
    clearRefTimeout(timerRef);
  };

  const isMouseEvent = useIsMouseEvent(viewport);

  useAsyncEffect(() => {
    const onPointerMove = (event: PointerEvent) => {
      if (isMouseEvent(event)) {
        reveal();
      }
    };

    viewport.addEventListener("pointermove", onPointerMove);
    return () => {
      viewport.removeEventListener("pointermove", onPointerMove);
      clearRefTimeout(timerRef);
    };
  }, [viewport]);

  // Keep hovered controls visibile indefinitely.
  const onPointerOver = (event: PointerEvent) => {
    if (isMouseEvent(event)) {
      hold();
    }
  };

  // Hide the chrome once the mouse is no longer over any of the chrome controls.
  const onPointerOut = (event: PointerEvent) => {
    if (!isMouseEvent(event)) return;

    if (
      // It's null when the mouse cursor leaves the document.
      !event.relatedTarget ||
      !(event.currentTarget as Node).contains(event.relatedTarget as Node)
    ) {
      reveal();
    }
  };

  return {
    visible,
    onPointerOver,
    onPointerOut,
  };
};

/** Helps figure out if the event is a mouse event, not a touch event. */
const useIsMouseEvent = (viewport: HTMLElement) => {
  const fromTouchRef = useRef(false);
  const touchTimerRef = useRef<number | null>(null);

  useAsyncEffect(() => {
    const onPointerDown = (event: PointerEvent) => {
      if (!isTrueMouseEvent(event)) {
        clearRefTimeout(touchTimerRef);
        fromTouchRef.current = true;
      }
    };
    const onPointerUp = (event: PointerEvent) => {
      if (!isTrueMouseEvent(event)) {
        // The imitation mouse events arrive after the finger lifts.
        clearRefTimeout(touchTimerRef);
        touchTimerRef.current = window.setTimeout(() => {
          touchTimerRef.current = null;
          fromTouchRef.current = false;
        }, TOUCH_TAIL_MS);
      }
    };

    viewport.addEventListener("pointerdown", onPointerDown);
    viewport.addEventListener("pointerup", onPointerUp);
    viewport.addEventListener("pointercancel", onPointerUp);
    return () => {
      viewport.removeEventListener("pointerdown", onPointerDown);
      viewport.removeEventListener("pointerup", onPointerUp);
      viewport.removeEventListener("pointercancel", onPointerUp);
      clearRefTimeout(touchTimerRef);
    };
  }, [viewport]);

  return function isMouseEvent(event: PointerEvent) {
    return isTrueMouseEvent(event) && !fromTouchRef.current;
  };
};

/** How long the chrome stays up after the last mouse move. */
const IDLE_MS = 1500;

/** How long imitation mouse events keep arriving after the finger lifts. */
const TOUCH_TAIL_MS = 700;
