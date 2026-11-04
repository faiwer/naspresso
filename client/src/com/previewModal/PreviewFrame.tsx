import clsx from "clsx";
import { useLayoutEffect, useRef } from "react";

import { getEntryFileUrl } from "../../api/storage";
import type { StorageFile } from "../../api/storage.io";
import type { FrameSide } from "./frameEnabled";
import { SETTLE_MS } from "./swipe/gesture";

import styles from "./PreviewFrame.module.css";

type PreviewFrameProps = {
  file: StorageFile;
  segments: string[];
  enabled: boolean;
  side: FrameSide;
  /** Extra x shift so a neighbor of a different width lands centered. */
  shift: number;
  settling: boolean;
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
  shift,
  settling,
  width,
  height,
  onLoad,
}: PreviewFrameProps) => {
  const imageRef = useRef<HTMLImageElement>(null);

  useLayoutEffect(() => {
    const image = imageRef.current;
    // A decoded image does not fire `load` again when its slot changes.
    if (enabled && image?.complete && image.naturalWidth > 0) {
      onLoad?.();
    }
  }, [enabled, onLoad]);

  return (
    <div
      className={clsx(styles.frame, styles[side])}
      style={{
        width,
        height,
        ...frameShift(side, shift, settling),
      }}
    >
      {enabled && (
        <img
          ref={imageRef}
          className={styles.image}
          // Pass it before `src` to ensure the cached images are handled correctly.
          onLoad={() => {
            onLoad?.();
            // Force early decoding to avoid layout thrashing when the swipe starts.
            imageRef.current!.decode();
          }}
          src={getEntryFileUrl(segments, file)}
          alt={file.name}
          draggable={false}
        />
      )}
    </div>
  );
};

/**
 * Provide an extra animation in addition to the settle animation. Why? To
 * compensate the discrepancy in width between the current image and the
 * neighbour image. Both animations must be consolidated.
 */
const frameShift = (
  side: FrameSide,
  shift: number,
  settling: boolean,
): { transform?: string; left?: string; transition?: string } => {
  if (side === "center" || (shift === 0 && !settling)) {
    return {}; // Nothing to compensate for.
  }

  // The CSS is not identical due to the initial styles.
  const value =
    side === "left"
      ? `translate(calc(-100% + ${shift}px), -50%)`
      : `calc(100% + ${shift}px)`;
  const transitionProp = side === "left" ? "transform" : "left";

  return settling
    ? // The final animation is in place. Compensate it with a transition animation.
      {
        [transitionProp]: value,
        transition: `${transitionProp} ${SETTLE_MS}ms ease-out`,
      }
    : { [transitionProp]: value };
};
