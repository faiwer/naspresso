import { DatabaseFilled } from "@ant-design/icons";
import { Breadcrumb } from "antd";
import { useState } from "react";
import { Link, useLocation } from "react-router";

import { Frame } from "../com/layout/Frame";
import { Gap } from "../com/layout/Gap";
import { PageTitle } from "../com/layout/PageTitle";
import { StorageListing } from "../com/storage/StorageListing";
import { StorageModeSwitch } from "../com/storage/StorageModeSwitch";
import {
  STORAGE_VIEW_MODE_KEY,
  StorageViewModeSchema,
} from "../com/storage/viewMode";
import { joinPath } from "../core/helpers/path";
import { useAsyncEffect } from "../core/hooks/useAsyncEffect";
import { useLocalStorageState } from "../core/hooks/useLocalStorageState";
import { parseStorageUrl, routes } from "../core/routes";

import styles from "./StoragePage.module.css";
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
  const rootLabel = (
    <span className={styles.rootCrumb}>
      <DatabaseFilled className={styles.rootIcon} />
      Storage
    </span>
  );
  const breadcrumbs = [rootLabel, ...segments].map((label, index) => ({
    title:
      index === segments.length ? (
        label
      ) : (
        <Link to={routes.storage(segments.slice(0, index))}>{label}</Link>
      ),
  }));

  // The container outlives directories, so it would keep the previous one's offset.
  useAsyncEffect(() => {
    scrollContainer?.scrollTo({ top: 0 });
  }, [path, mode]);

  return (
    <Frame
      scrollContainerRef={setScrollContainer}
      header={
        <>
          <Breadcrumb items={breadcrumbs} />
          <Gap />
          <StorageModeSwitch value={mode} onChange={setMode} />
        </>
      }
    >
      <PageTitle title={segments.at(-1) ?? "Storage"} />
      {scrollContainer && (
        <StorageListing
          key={path}
          segments={segments}
          mode={mode}
          scrollContainer={scrollContainer}
        />
      )}
    </Frame>
  );
};
