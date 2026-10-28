import { FolderOpenFilled } from "@ant-design/icons";
import { Link } from "react-router";

import { getFileUrl } from "../../api/storage";
import type { StorageDir, StorageEntry } from "../../api/storage.io";
import { joinPath } from "../../core/helpers/path";
import { routes } from "../../core/routes";
import type { StorageViewProps } from "./StorageList";

import styles from "./StorageThumbnails.module.css";

export const StorageThumbnails = ({ segments, entries }: StorageViewProps) => (
  <div className={styles.grid}>
    {segments.length > 0 && (
      <Thumbnail entry={PARENT_ENTRY} segments={segments.slice(0, -1)} />
    )}
    {entries.map((entry) => (
      <Thumbnail
        key={entry.name}
        entry={entry}
        segments={[...segments, entry.name]}
      />
    ))}
  </div>
);

type ThumbnailProps = {
  entry: StorageEntry;
  /** Where the tile points: the entry's own path, or the parent directory for `..`. */
  segments: string[];
};

/** The `..` tile; it isn't a real entry, so its timestamps are dummies. */
const PARENT_ENTRY: StorageDir = {
  kind: "dir",
  name: "..",
  modifiedAt: 0,
  createdAt: null,
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
