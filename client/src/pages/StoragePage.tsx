import { Breadcrumb } from "antd";
import { useState } from "react";
import { useLocation } from "react-router";

import { Frame } from "../com/layout/Frame";
import { Gap } from "../com/layout/Gap";
import { PageTitle } from "../com/layout/PageTitle";
import { makeSegmentBreadcrumbs } from "../com/storage/SegmentBreadcrumb";
import { StorageListing } from "../com/storage/StorageListing";
import { StorageModeSwitch } from "../com/storage/StorageModeSwitch";
import { useStorageFeed } from "../com/storage/useStorageFeed";
import {
  STORAGE_VIEW_MODE_KEY,
  StorageViewModeSchema,
} from "../com/storage/viewMode";
import { joinPath } from "../core/helpers/path";
import { useAsyncEffect } from "../core/hooks/useAsyncEffect";
import { useLocalStorageState } from "../core/hooks/useLocalStorageState";
import { parseStorageUrl } from "../core/routes";

/**
 * Lists a directory of `STORAGE`. Mounted at `/storage/*`; the rest of the URL is the
 * directory path, built by `routes.storage`.
 */
export const StoragePage = () => {
  const segments = parseStorageUrl(useLocation().pathname);
  const path = joinPath(segments);
  const [mode, setMode] = useLocalStorageState(
    STORAGE_VIEW_MODE_KEY,
    StorageViewModeSchema,
    "thumbnails",
  );
  const [scrollContainer, setScrollContainer] = useState<HTMLElement | null>(
    null,
  );
  const feed = useStorageFeed(joinPath(segments));

  // The container outlives directories, so it would keep the previous one's offset.
  useAsyncEffect(() => {
    scrollContainer?.scrollTo({ top: 0 });
  }, [path, mode]);

  return (
    <Frame
      scrollContainerRef={setScrollContainer}
      header={
        <>
          <Breadcrumb
            items={makeSegmentBreadcrumbs(segments, feed.encryption)}
          />
          <Gap />
          <StorageModeSwitch value={mode} onChange={setMode} />
        </>
      }
    >
      <PageTitle title={segments.at(-1) ?? "Storage"} />
      {scrollContainer && (
        <StorageListing
          segments={segments}
          feed={feed}
          mode={mode}
          scrollContainer={scrollContainer}
        />
      )}
    </Frame>
  );
};
