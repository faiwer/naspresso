import { useRef, useState } from "react";

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

  useAsyncEffect(() => {
    const onMouseMove = (event: MouseEvent) => {
      // A held button is a swipe.
      if (event.buttons !== 0) return;
      reveal();
    };

    viewport.addEventListener("mousemove", onMouseMove);
    return () => {
      viewport.removeEventListener("mousemove", onMouseMove);
      clearRefTimeout(timerRef);
    };
  }, [viewport]);

  const onMouseOut = (event: MouseEvent) => {
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
    onMouseOver: hold,
    onMouseOut,
  };
};

/** How long the chrome stays up after the last mouse move. */
const IDLE_MS = 1500;
