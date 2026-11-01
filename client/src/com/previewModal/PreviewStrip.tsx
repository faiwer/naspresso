import { useRef, useStableCallback } from "react";

import type { StorageFile } from "../../api/storage.io";
import { nullthrows } from "../../core/helpers/nullthrows";
import { useAsyncEffect } from "../../core/hooks/useAsyncEffect";
import { fittedSize } from "./fittedSize";
import { isFrameEnabled, type FrameSide } from "./frameEnabled";
import { PreviewFrame } from "./PreviewFrame";
import {
  neighborShift,
  SETTLE_MS,
  getSnapDistance,
  type SideDownloaded,
} from "./swipe/gesture";

import styles from "./PreviewModal.module.css";

type PreviewStripProps = {
  files: StorageFile[];
  pos: number;
  segments: string[];
  viewport: { width: number; height: number };
  translateX: number;
  settling: boolean;
  promoted: FrameSide | null;
  downloaded: SideDownloaded;
  onDownloaded: (side: keyof SideDownloaded) => void;
  /** The settle animation finished, or the transform never changed. */
  onSettled: () => void;
};

/** The current image and the neighbors flush with the viewport edges. */
export const PreviewStrip = ({
  files,
  pos,
  segments,
  viewport,
  translateX,
  settling,
  promoted,
  downloaded,
  onDownloaded,
  onSettled,
}: PreviewStripProps) => {
  const centerFile = nullthrows(files[pos]);
  const leftFile = files[pos - 1] ?? null;
  const rightFile = files[pos + 1] ?? null;

  const centerBox = fittedBox(centerFile, viewport);
  const leftBox = leftFile ? nullthrows(fittedBox(leftFile, viewport)) : null;
  const rightBox = rightFile
    ? nullthrows(fittedBox(rightFile, viewport))
    : null;

  const snapDistance = getSnapDistance(viewport.width, centerBox.width);
  const leftShift =
    leftBox == null
      ? 0
      : neighborShift(
          translateX,
          snapDistance,
          centerBox.width,
          leftBox.width,
          "left",
        );
  const rightShift =
    rightBox == null
      ? 0
      : neighborShift(
          translateX,
          snapDistance,
          centerBox.width,
          rightBox.width,
          "right",
        );

  const markLeft = useStableCallback(() => onDownloaded("left"));
  const markCenter = useStableCallback(() => onDownloaded("center"));
  const markRight = useStableCallback(() => onDownloaded("right"));

  const trackRef = useRef<HTMLDivElement>(null);
  useAsyncEffect(() => {
    if (settling && !hasRunningTransform(nullthrows(trackRef.current))) {
      // Normally, the browser would fire `onTransitionEnd` when the animation
      // ends. But when the animation is not running, we need to manually call
      // `onSettled`. How can it happen? Accidentally, e.g. when `translateX`
      // of both steps of the swipe were identical. Zero value change let to
      // zero animation.
      onSettled();
    }
  }, [settling]);

  return (
    <div
      ref={trackRef}
      className={styles.track}
      onTransitionEnd={(event) => {
        if (
          event.target === event.currentTarget &&
          event.propertyName === "transform"
        ) {
          onSettled();
        }
      }}
      style={{
        transform: `translateX(${translateX}px)`,
        ...(settling
          ? { transition: `transform ${SETTLE_MS}ms ease-out` }
          : null),
      }}
    >
      {leftFile && (
        <PreviewFrame
          key={leftFile.name}
          file={leftFile}
          segments={segments}
          side="left"
          shift={leftShift}
          settling={settling}
          enabled={isFrameEnabled(
            "left",
            promoted,
            downloaded,
            rightFile != null,
          )}
          onLoad={markLeft}
          {...nullthrows(leftBox)}
        />
      )}
      <PreviewFrame
        key={centerFile.name}
        file={centerFile}
        segments={segments}
        side="center"
        shift={0}
        settling={settling}
        enabled
        onLoad={markCenter}
        {...centerBox}
      />
      {rightFile && (
        <PreviewFrame
          key={rightFile.name}
          file={rightFile}
          segments={segments}
          side="right"
          shift={rightShift}
          settling={settling}
          enabled={isFrameEnabled("right", promoted, downloaded, true)}
          onLoad={markRight}
          {...nullthrows(rightBox)}
        />
      )}
    </div>
  );
};

const fittedBox = (
  file: StorageFile,
  viewport: { width: number; height: number },
) => fittedSize(nullthrows(file.image), viewport);

/** True when a CSS `transform` transition is still in progress on `element`. */
const hasRunningTransform = (element: Element): boolean =>
  element
    .getAnimations()
    .some(
      (animation) =>
        animation instanceof CSSTransition &&
        animation.transitionProperty === "transform" &&
        animation.playState !== "finished" &&
        animation.playState !== "idle",
    );
