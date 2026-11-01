import {
  useLayoutEffect,
  useRef,
  useState,
  useStableCallback,
  useEffect,
} from "react";

import { nullthrows } from "../../core/helpers/nullthrows";
import { updateIfChanged } from "../../core/helpers/object";
import type { PreviewSession } from "./context";
import { PreviewStrip } from "./PreviewStrip";

import styles from "./PreviewModal.module.css";

type PreviewModalProps = {
  session: PreviewSession;
  onClose: () => void;
};

/** Full-screen preview of the original file at `session.pos`. */
export const PreviewModal = ({ session, onClose }: PreviewModalProps) => {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const file = nullthrows(session.files[session.pos]);
  const [viewport, setViewport] = useState<Viewport | null>(null);

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
            pos={session.pos}
            segments={session.segments}
            viewport={viewport}
          />
        )}
      </div>
    </dialog>
  );
};

type Viewport = {
  element: HTMLDivElement;
  width: number;
  height: number;
};
