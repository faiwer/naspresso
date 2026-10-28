import { Breadcrumb } from "antd";
import { type ReactNode, useState } from "react";
import { Link, useLocation } from "react-router";

import { getListStorage } from "../api/storage";
import { Frame } from "../com/layout/Frame";
import { Gap } from "../com/layout/Gap";
import { StorageList, type StorageViewProps } from "../com/storage/StorageList";
import {
  StorageModeSwitch,
  type StorageViewMode,
} from "../com/storage/StorageModeSwitch";
import { StorageTable } from "../com/storage/StorageTable";
import { StorageThumbnails } from "../com/storage/StorageThumbnails";
import { useAsyncData } from "../core/hooks/useAsyncData";
import { parseStorageUrl, routes } from "../core/routes";
/**
 * Lists a directory of `STORAGE`. Mounted at `/storage/*`; the rest of the URL is the
 * directory path, built by `routes.storage`.
 */
export const StoragePage = () => {
  const segments = parseStorageUrl(useLocation().pathname);
  const path = "/" + segments.join("/");
  const listing = useAsyncData(
    (signal) => getListStorage(path, { signal }),
    [path],
  );
  const [mode, setMode] = useState<StorageViewMode>("thumbnails");
  const View = VIEWS[mode];
  const breadcrumbs = ["Storage", ...segments].map((name, index) => ({
    title:
      index === segments.length ? (
        name
      ) : (
        <Link to={routes.storage(segments.slice(0, index))}>{name}</Link>
      ),
  }));

  return (
    <Frame
      header={
        <>
          <Breadcrumb items={breadcrumbs} />
          <Gap />
          <StorageModeSwitch value={mode} onChange={setMode} />
        </>
      }
    >
      {listing.isLoading ? (
        <p>…</p>
      ) : listing.error ? (
        <p>{listing.error.message}</p>
      ) : (
        <View segments={segments} entries={listing.data} />
      )}
    </Frame>
  );
};

const VIEWS: Record<StorageViewMode, (props: StorageViewProps) => ReactNode> = {
  list: StorageList,
  table: StorageTable,
  thumbnails: StorageThumbnails,
};
