import { useLayoutEffect, useRef } from "react";

import { getEntryFileUrl } from "../../api/storage";
import { nullthrows } from "../../core/helpers/nullthrows";
import type { PreviewSession } from "./context";

import styles from "./PreviewModal.module.css";

type PreviewModalProps = {
  session: PreviewSession;
  onClose: () => void;
};

/** Full-screen preview of the original file at `session.pos`. */
export const PreviewModal = ({ session, onClose }: PreviewModalProps) => {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const file = nullthrows(session.files[session.pos]);

  useLayoutEffect(() => {
    const dialog = nullthrows(dialogRef.current);
    dialog.showModal();
    return () => {
      dialog.close();
    };
  }, []);

  return (
    <dialog
      ref={dialogRef}
      className={styles.overlay}
      aria-label={file.name}
      onClick={(event) => {
        if (event.target === event.currentTarget) {
          onClose();
        }
      }}
      onCancel={(event) => {
        event.preventDefault();
        onClose();
      }}
    >
      <img
        className={styles.image}
        src={getEntryFileUrl(session.segments, file.name)}
        alt={file.name}
        draggable={false}
      />
    </dialog>
  );
};
