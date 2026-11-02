import {
  useLayoutEffect,
  useRef,
  useState,
  useStableCallback,
  useEffect,
} from "react";

import { formatKeyChord } from "../../core/helpers/keyboard";
import { nullthrows } from "../../core/helpers/nullthrows";
import { updateIfChanged } from "../../core/helpers/object";
import type { PreviewSession } from "./context";
import { PreviewControls } from "./controls/PreviewControls";
import { PreviewStrip } from "./PreviewStrip";
import { usePreviewSwipe } from "./swipe/usePreviewSwipe";

import styles from "./PreviewModal.module.css";

type PreviewModalProps = {
  session: PreviewSession;
  onClose: () => void;
};

/** Full-screen preview of one image from `session`. */
export const PreviewModal = ({ session, onClose }: PreviewModalProps) => {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const [viewport, setViewport] = useState<Viewport | null>(null);
  const [pos, setPos] = useState(session.pos);
  const swipe = usePreviewSwipe(session.files, pos, setPos, viewport);
  const file = nullthrows(session.files[pos]);

  useLayoutEffect(() => {
    const dialog = nullthrows(dialogRef.current);
    dialog.showModal();
    return () => void dialog.close();
  }, []);

  useEffect(() => {
    const element = viewport?.element;
    if (!element) return;

    const update = () =>
      void setViewport((prev) =>
        updateIfChanged(prev, {
          element,
          width: element.clientWidth,
          height: element.clientHeight,
        }),
      );

    update();
    const observer = new ResizeObserver(update);
    observer.observe(element);
    return () => observer.disconnect();
  }, [viewport?.element]);

  return (
    <dialog
      ref={dialogRef}
      className={styles.overlay}
      aria-label={file.name}
      onCancel={(event) => {
        event.preventDefault();
        onClose();
      }}
      onKeyDown={(event) => {
        const chord = formatKeyChord(event.nativeEvent);
        if (chord !== "ArrowLeft" && chord !== "ArrowRight") return;
        event.preventDefault();
        swipe.navigate(chord === "ArrowLeft" ? -1 : 1);
      }}
      onPointerDown={swipe.onPointerDown}
      onPointerMove={swipe.onPointerMove}
      onPointerUp={swipe.onPointerUp}
      onPointerCancel={swipe.onPointerCancel}
      onContextmenu={(event) => event.preventDefault()}
    >
      <div
        ref={useStableCallback((element) =>
          setViewport(
            element
              ? {
                  element,
                  width: element.clientWidth,
                  height: element.clientHeight,
                }
              : null,
          ),
        )}
        className={styles.viewport}
        onClick={(event) => {
          if (event.target === event.currentTarget) {
            onClose();
          }
        }}
      >
        {viewport && (
          <PreviewStrip
            files={session.files}
            pos={pos}
            segments={session.segments}
            viewport={viewport}
            translateX={swipe.translateX}
            settling={swipe.settling}
            promoted={swipe.promoted}
            downloaded={swipe.downloaded}
            onDownloaded={swipe.markDownloaded}
            onSettled={swipe.onSettled}
          />
        )}
      </div>
      {viewport && (
        <PreviewControls
          viewport={viewport.element}
          pos={pos}
          count={session.files.length}
          onNavigate={swipe.navigate}
        />
      )}
    </dialog>
  );
};

type Viewport = {
  element: HTMLDivElement;
  width: number;
  height: number;
};
