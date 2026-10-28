import { Button } from "antd";
import type { ReactNode } from "react";

import { joinPath } from "../../core/helpers/path";
import { ScrollSentinel } from "../layout/ScrollSentinel";
import { StorageList, type StorageViewProps } from "./StorageList";
import type { StorageViewMode } from "./StorageModeSwitch";
import { StorageTable } from "./StorageTable";
import { StorageThumbnails } from "./StorageThumbnails";
import { STORAGE_PAGE_SIZE, useStorageFeed } from "./useStorageFeed";

type StorageListingProps = {
  /** The listed directory as path segments; empty for the root. */
  segments: string[];
  mode: StorageViewMode;
  /** The element the listing scrolls in; more is loaded as it nears the bottom. */
  scrollContainer: HTMLElement;
};

/**
 * A directory's entries, loaded a page at a time as the user scrolls. It lists one
 * directory for its whole life: remount it (`key`) to list another.
 */
export const StorageListing = ({
  segments,
  mode,
  scrollContainer,
}: StorageListingProps) => {
  const feed = useStorageFeed(joinPath(segments));
  const View = VIEWS[mode];
  const skeletons = !feed.isLoading
    ? 0
    : feed.total == null
      ? STORAGE_PAGE_SIZE
      : Math.min(STORAGE_PAGE_SIZE, feed.total - feed.items.length);

  return (
    <>
      <View segments={segments} entries={feed.items} skeletons={skeletons} />
      {feed.error && (
        <p>
          {feed.error.message}{" "}
          <Button size="small" onClick={feed.retry}>
            Retry
          </Button>
        </p>
      )}
      {/* Unmounted while a page loads, so each page gets a fresh observer. */}
      {feed.canLoadMore && (
        <ScrollSentinel root={scrollContainer} onReached={feed.loadMore} />
      )}
    </>
  );
};

const VIEWS: Record<StorageViewMode, (props: StorageViewProps) => ReactNode> = {
  list: StorageList,
  table: StorageTable,
  thumbnails: StorageThumbnails,
};
