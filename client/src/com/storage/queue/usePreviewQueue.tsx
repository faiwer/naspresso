import { useLayoutEffect, useState } from "react";

import { PreviewQueueContext } from "./context";
import { PreviewQueue } from "./PreviewQueue";

type PreviewQueueProviderProps = {
  queue: PreviewQueue;
  children: JSX.Element;
};

/** Makes `queue` available to {@link usePreviewJob}. */
export const PreviewQueueProvider = ({
  queue,
  children,
}: PreviewQueueProviderProps) => (
  <PreviewQueueContext.Provider value={queue}>
    {children}
  </PreviewQueueContext.Provider>
);

/**
 * The listing's preview queue.
 *
 * Masonry and thumbnails render every tile, and a tile's `<img>` starts
 * its request as soon as it mounts. This hook keeps that from fetching the
 * whole folder: one {@link PreviewQueue} on the frame scroller, shared by
 * every {@link usePreviewJob} under it.
 *
 * Only uncached previews join. The observer watches a band of
 * {@link PreviewQueue.ROOT_MARGIN} around the scrollport. Waiting jobs
 * start on screen, then below it, then above it, in feed order, and at
 * most {@link PreviewQueue.PREVIEW_SLOTS} of them download at once.
 * Scrolling re-ranks the ones still waiting. A download that has already
 * started keeps its slot until it finishes, fails, or leaves the band.
 * Leaving the band aborts it, so the server can drop a preview it has not
 * started rendering and spend that slot on a tile that is still on screen.
 * A failed preview is not tried again.
 *
 * `root` is read once, on the first render. Remount the listing to point
 * the queue at a different scroll element.
 */
export const usePreviewQueue = (root: HTMLElement): PreviewQueue => {
  const [queue] = useState(() => new PreviewQueue(root));

  useLayoutEffect(() => () => queue.disconnect(), [queue]);

  return queue;
};
