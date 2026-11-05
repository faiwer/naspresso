import type { ReactNode } from "react";

import { ErrorToast } from "../feedback/ErrorToast";
import { ScrollSentinel } from "../layout/ScrollSentinel";
import { StorageMasonry } from "./masonry/StorageMasonry";
import { StorageList, type StorageViewProps } from "./StorageList";
import { StorageTable } from "./StorageTable";
import { StorageThumbnails } from "./StorageThumbnails";
import { useOpenStorageImage } from "./useOpenStorageImage";
import { STORAGE_PAGE_SIZE, type StorageFeed } from "./useStorageFeed";
import type { StorageViewMode } from "./viewMode";

type StorageListingProps = {
  /** The listed directory as path segments; empty for the root. */
  segments: string[];
  mode: StorageViewMode;
  /** The element the listing scrolls in; more is loaded as it nears the bottom. */
  scrollContainer: HTMLElement;
  feed: StorageFeed;
};

/**
 * A directory's entries, loaded a page at a time as the user scrolls. It lists one
 * directory for its whole life: remount it (`key`) to list another.
 */
export const StorageListing = ({
  segments,
  mode,
  scrollContainer,
  feed,
}: StorageListingProps) => {
  const openImage = useOpenStorageImage(segments, feed.items);
  const View = VIEWS[mode];
  const skeletons = !feed.isLoading
    ? 0
    : feed.total == null
      ? STORAGE_PAGE_SIZE
      : Math.min(STORAGE_PAGE_SIZE, feed.total - feed.items.length);

  return (
    <>
      <View
        segments={segments}
        entries={feed.items}
        skeletons={skeletons}
        scrollContainer={scrollContainer}
        openImage={openImage}
        encryption={feed.encryption}
      />
      {feed.error && <ErrorToast error={feed.error} onRetry={feed.retry} />}
      {/* Unmounted while a page loads, so each page gets a fresh observer. */}
      {feed.canLoadMore && (
        <ScrollSentinel root={scrollContainer} onReached={feed.loadMore} />
      )}
    </>
  );
};

const VIEWS: Record<StorageViewMode, (props: StorageViewProps) => ReactNode> = {
  list: StorageList,
  masonry: StorageMasonry,
  table: StorageTable,
  thumbnails: StorageThumbnails,
};
