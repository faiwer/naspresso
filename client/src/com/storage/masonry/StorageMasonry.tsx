import { useLayoutEffect, useMemo, useRef, useState } from "react";

import { rootFontSize } from "../../../core/helpers/browser";
import type { StorageViewProps } from "../StorageList";
import {
  MASONRY_GAP_PX,
  MASONRY_MIN_COLUMN_REM,
  masonryColumnCount,
  masonryTileAspect,
  packMasonryColumns,
} from "./packMasonryColumns";
import { MasonrySkeleton, useMasonrySkeletons } from "./skeletons";
import { Tile } from "./Tile";

import styles from "./StorageMasonry.module.css";

export const StorageMasonry = ({
  segments,
  entries,
  skeletons,
}: StorageViewProps) => {
  const rootRef = useRef<HTMLDivElement>(null);
  const [box, setBox] = useState({ width: 0, rem: 16 });

  useLayoutEffect(() => {
    const root = rootRef.current!;

    const update = () => {
      const width = root.clientWidth;
      const rem = rootFontSize();
      setBox((prev) =>
        prev.width === width && prev.rem === rem ? prev : { width, rem },
      );
    };
    update();
    const observer = new ResizeObserver(update);
    observer.observe(root);
    return () => observer.disconnect();
  }, []);

  const { width, rem } = box;
  const columnCount = masonryColumnCount(
    width,
    MASONRY_MIN_COLUMN_REM * rem,
    MASONRY_GAP_PX,
  );
  const columnWidth =
    (width - MASONRY_GAP_PX * (columnCount - 1)) / columnCount;
  const { columns, heights } = useMemo(
    () =>
      packMasonryColumns(
        entries,
        columnCount,
        (entry) => masonryTileAspect(entry) * columnWidth + MASONRY_GAP_PX,
      ),
    [entries, columnCount, columnWidth],
  );
  const skeletonSlots = useMasonrySkeletons(
    skeletons,
    heights,
    columnWidth,
    MASONRY_GAP_PX,
  );

  return (
    <div ref={rootRef} className={styles.root}>
      {columns.map((column, col) => (
        <div key={col} className={styles.column}>
          {column.map((entry) => (
            <Tile
              key={entry.name}
              entry={entry}
              segments={[...segments, entry.name]}
            />
          ))}
          {(skeletonSlots[col] ?? []).map((index) => (
            <MasonrySkeleton key={`skeleton-${index}`} index={index} />
          ))}
        </div>
      ))}
    </div>
  );
};
