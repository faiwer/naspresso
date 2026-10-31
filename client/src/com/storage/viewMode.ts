import { z } from "zod";

/** `localStorage` key for the storage listing's view mode. */
export const STORAGE_VIEW_MODE_KEY = "storage.viewMode";

export const StorageViewModeSchema = z.enum([
  "list",
  "masonry",
  "table",
  "thumbnails",
]);

export type StorageViewMode = z.infer<typeof StorageViewModeSchema>;
