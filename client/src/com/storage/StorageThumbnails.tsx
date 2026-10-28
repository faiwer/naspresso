import { FolderOpenFilled } from "@ant-design/icons";
import { Skeleton } from "antd";
import { Link } from "react-router";

import { getFileUrl } from "../../api/storage";
import type { StorageEntry } from "../../api/storage.io";
import { range } from "../../core/helpers/array";
import { joinPath } from "../../core/helpers/path";
import { routes } from "../../core/routes";
import type { StorageViewProps } from "./StorageList";

import styles from "./StorageThumbnails.module.css";

export const StorageThumbnails = ({
  segments,
  entries,
  skeletons,
}: StorageViewProps) => (
  <div className={styles.grid}>
    {entries.map((entry) => (
      <Thumbnail
        key={entry.name}
        entry={entry}
        segments={[...segments, entry.name]}
      />
    ))}
    {range(skeletons).map((index) => (
      <ThumbnailSkeleton key={`skeleton-${index}`} />
    ))}
  </div>
);

type ThumbnailProps = {
  entry: StorageEntry;
  /** The entry's own path. */
  segments: string[];
};

const Thumbnail = ({ entry, segments }: ThumbnailProps) => {
  const { name } = entry;
  const content = (
    <>
      {entry.kind === "dir" ? (
        <div className={styles.folder}>
          <FolderOpenFilled />
        </div>
      ) : (
        <div className={styles.preview} />
      )}
      <div className={styles.name} title={name}>
        {name}
      </div>
    </>
  );

  if (entry.kind === "dir") {
    return (
      <Link className={styles.item} to={routes.storage(segments)}>
        {content}
      </Link>
    );
  }
  return (
    <a
      className={styles.item}
      href={getFileUrl(joinPath(segments))}
      target="_blank"
      rel="noreferrer"
    >
      {content}
    </a>
  );
};

const ThumbnailSkeleton = () => (
  <div className={styles.skeleton}>
    <Skeleton.Node active styles={PREVIEW_SKELETON_STYLES} />
    <Skeleton.Input active block size="small" className={styles.skeletonName} />
  </div>
);

/** Stretches the node to the tile's width, square like a preview. */
const PREVIEW_SKELETON_STYLES = {
  root: { display: "block" },
  content: { width: "100%", height: "auto", aspectRatio: "1" },
};
