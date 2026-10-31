import {
  useMemo,
  useStableCallback,
  useSyncExternalStore,
  type RefObject,
} from "react";

import { nullthrows } from "../../../core/helpers/nullthrows";
import { useAsyncEffect } from "../../../core/hooks/useAsyncEffect";
import { usePreviewQueueContextX } from "./context";
import type { PreviewSnapshot } from "./types";

export type PreviewJob = {
  enabled: boolean;
  onLoad: () => void;
  onError: () => void;
};

/**
 * Registers an uncached preview with the queue. A cached one stays enabled
 * and is not observed. `onLoad` and `onError` resolve the attempt they
 * closed over.
 */
export const usePreviewJob = (
  element: RefObject<HTMLDivElement | null>,
  filePath: string,
  index: number,
  hasPreview: boolean,
): PreviewJob => {
  const queue = usePreviewQueueContextX();

  const snap = useSyncExternalStore(
    useMemo(
      () =>
        hasPreview
          ? () => () => {}
          : (onStoreChange: () => void) =>
              queue.subscribe(filePath, onStoreChange),
      [hasPreview, queue, filePath],
    ),
    useMemo(
      () => (hasPreview ? () => CACHED : () => queue.snapshot(filePath)),
      [hasPreview, queue, filePath],
    ),
  );

  useAsyncEffect(() => {
    if (hasPreview) {
      return queue.attach(nullthrows(element.current), {
        id: filePath,
        index,
      });
    }
  }, [hasPreview, queue, filePath, index]);

  return {
    enabled: snap.enabled,
    onLoad: useStableCallback(() => {
      if (hasPreview) {
        queue.settle(filePath, snap.attempt);
      }
    }),
    onError: useStableCallback(() => {
      if (hasPreview) {
        queue.settle(filePath, snap.attempt);
      }
    }),
  };
};

/** A cached preview is shown as soon as its tile mounts. */
const CACHED: PreviewSnapshot = { enabled: true, attempt: 0 };
