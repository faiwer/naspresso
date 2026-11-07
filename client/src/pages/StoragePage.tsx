import { Breadcrumb } from "antd";
import { useState } from "react";
import { useLocation } from "react-router";

import { isWithinEncryptedFolder } from "../api/storage";
import { Frame } from "../com/layout/Frame";
import { Gap } from "../com/layout/Gap";
import { PageTitle } from "../com/layout/PageTitle";
import { EncryptionPanel } from "../com/storage/encryption/EncryptionPanel";
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
  const feed = useStorageFeed(path);

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
      {isWithinEncryptedFolder(segments) && (
        <EncryptionPanel
          key={path}
          segments={segments}
          encryption={feed.encryption}
          lockAt={feed.lockAt}
          onUnlocked={feed.unlock}
          onLocked={feed.lock}
          onPostponed={feed.postpone}
        />
      )}
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
