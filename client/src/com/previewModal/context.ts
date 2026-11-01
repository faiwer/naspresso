import { createContext, useContext } from "react";

import type { StorageFile } from "../../api/storage.io";
import { nullthrows } from "../../core/helpers/nullthrows";

/** One open preview: the caller's image files and which one is showing. */
export type PreviewSession = {
  files: StorageFile[];
  /** Index into `files`. */
  pos: number;
  /** Directory that contains every file, as path segments. */
  segments: string[];
};

export type PreviewModalApi = {
  /**
   * Opens the preview on `files[pos]`. `segments` is the directory that
   * contains every file, used to build its download URL.
   */
  openPreviewModal: (
    files: StorageFile[],
    pos: number,
    segments: string[],
  ) => void;
};

export const PreviewModalContext = createContext<PreviewModalApi | null>(null);

/** The app preview modal. Throws outside {@link PreviewModalProvider}. */
export const usePreviewModalContextX = (): PreviewModalApi =>
  nullthrows(
    useContext(PreviewModalContext),
    "usePreviewModalContextX must be used inside PreviewModalProvider",
  );
