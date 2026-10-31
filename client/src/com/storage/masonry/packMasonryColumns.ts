import type { StorageEntry } from "../../../api/storage.io";

/** Space between columns and stacked tiles; keep in sync with the CSS `gap`. */
export const MASONRY_GAP_PX = 2;

/** Narrowest a column may be before the count drops. */
export const MASONRY_MIN_COLUMN_REM = 15;

/** How many equal columns fit in `width`. */
export const masonryColumnCount = (
  width: number,
  minColumn: number,
  gap: number,
): number => Math.max(1, Math.floor((width + gap) / (minColumn + gap)));

/**
 * Assigns each item to the currently shortest column. Prefix-stable: packing
 * `items` then packing `items` plus more leaves the first assignments in place.
 */
export const packMasonryColumns = <T>(
  items: T[],
  columnCount: number,
  heightOf: (item: T) => number,
): { columns: T[][]; heights: number[] } => {
  const columns: T[][] = Array.from({ length: columnCount }, () => []);
  const heights = Array.from({ length: columnCount }, () => 0);
  for (const item of items) {
    const col = shortestColumnIndex(heights);
    columns[col].push(item);
    heights[col] += heightOf(item);
  }
  return { columns, heights };
};

export const shortestColumnIndex = (heights: number[]): number => {
  let best = 0;
  for (let i = 1; i < heights.length; i++) {
    if (heights[i] < heights[best]) {
      best = i;
    }
  }
  return best;
};

/** Height / width of a folder or non-image tile (`aspect-ratio: 4 / 2`). */
const MASONRY_ICON_ASPECT = 2 / 4;

/** Image aspect, or 4:2 for folders and non-images. */
export const masonryTileAspect = (entry: StorageEntry): number => {
  if (entry.kind === "file" && entry.image != null && entry.image.width > 0) {
    return entry.image.height / entry.image.width;
  }
  return MASONRY_ICON_ASPECT;
};
