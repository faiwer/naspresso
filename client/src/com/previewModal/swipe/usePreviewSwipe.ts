import { useState, useStableCallback } from "react";

import type { StorageFile } from "../../../api/storage.io";
import {
  commitTranslate,
  getSnapDistance,
  type SideDownloaded,
} from "./gesture";
import { initialMotion, type Motion } from "./motion";
import { useDragGesture } from "./useDragGesture";
import { frameWidth, neighborAt, type Viewport } from "./widths";

/** Touch swipe between the images already in the session. */
export const usePreviewSwipe = (
  files: StorageFile[],
  pos: number,
  setPos: (pos: number) => void,
  viewport: Viewport | null,
) => {
  const [motion, setMotion] = useState(initialMotion);

  const gesture = useDragGesture({
    files,
    pos,
    viewport,
    settling: motion.settling,
    setMotion,
  });

  const markDownloaded = useStableCallback((side: keyof SideDownloaded) => {
    setMotion((prev) => ({
      ...prev,
      downloaded: { ...prev.downloaded, [side]: true },
    }));
  });

  // Callback for non-swipe navigation. Just to reuse the same animation code.
  const navigate = useStableCallback((delta: -1 | 1) => {
    // Block any navigation during already ongoing animation.
    if (motion.settling || viewport == null) return;

    // Positive dx moves toward the previous image.
    const dx = delta > 0 ? -1 : 1;
    const neighbor = neighborAt(files, pos, dx, viewport);
    // The 1st or the last image has no neighbor.
    if (neighbor == null) return;

    setMotion((prev) => ({
      ...prev,
      translateX: commitTranslate(
        delta,
        getSnapDistance(viewport.width, frameWidth(files[pos], viewport)),
      ),
      settling: true,
      delta,
      promoted: neighbor.side,
    }));
  });

  return {
    translateX: motion.translateX,
    settling: motion.settling,
    promoted: motion.promoted,
    downloaded: motion.downloaded,
    markDownloaded,
    navigate,
    onSettled: useOnSettled(pos, setMotion, setPos),
    ...gesture,
  };
};

const useOnSettled = (
  pos: number,
  setMotion: (update: (motion: Motion) => Motion) => void,
  setPos: (pos: number) => void,
) => {
  return useStableCallback(() => {
    let delta: -1 | 0 | 1;

    setMotion((prev) => {
      delta = prev.delta;
      return delta === 0
        ? { ...initialMotion, downloaded: prev.downloaded }
        : initialMotion;
    });

    // The swipe is done. Update the position.
    setPos(pos + delta!);
  });
};
