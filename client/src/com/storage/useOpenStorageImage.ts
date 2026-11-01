import { useStableCallback } from "react";

import type { StorageEntry, StorageFile } from "../../api/storage.io";
import { usePreviewModalContextX } from "../previewModal/context";

/**
 * Opens `file` in the preview modal at its place among the images in `entries`.
 * `segments` is the directory that contains them.
 */
export const useOpenStorageImage = (
  segments: string[],
  entries: StorageEntry[],
) => {
  const { openPreviewModal } = usePreviewModalContextX();
  return useStableCallback((file: StorageFile) => {
    const files = entries.filter(isPreviewImage);
    const pos = files.findIndex((item) => item.name === file.name);
    if (pos >= 0) {
      openPreviewModal(files, pos, segments);
    }
  });
};

/** An image the preview can show, including one whose headers could not be read. */
export const isPreviewImage = (entry: StorageEntry): entry is StorageFile =>
  entry.kind === "file" && entry.image != null;
