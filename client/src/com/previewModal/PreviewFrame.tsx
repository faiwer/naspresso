import clsx from "clsx";

import { getEntryFileUrl } from "../../api/storage";
import type { StorageFile } from "../../api/storage.io";
import type { FrameSide } from "./frameEnabled";

import styles from "./PreviewFrame.module.css";

type PreviewFrameProps = {
  file: StorageFile;
  segments: string[];
  enabled: boolean;
  side: FrameSide;
  width: number;
  height: number;
  /** Fires once the image has downloaded, including from cache. */
  onLoad?: () => void;
};

/** One image slot. The picture is requested only while `enabled`. */
export const PreviewFrame = ({
  file,
  segments,
  enabled,
  side,
  width,
  height,
  onLoad,
}: PreviewFrameProps) => {
  return (
    <div className={clsx(styles.frame, styles[side])} style={{ width, height }}>
      {enabled && (
        <img
          className={styles.image}
          // Pass it before `src` to ensure the cached images are handled correctly.
          onLoad={onLoad}
          src={getEntryFileUrl(segments, file.name)}
          alt={file.name}
          draggable={false}
        />
      )}
    </div>
  );
};
