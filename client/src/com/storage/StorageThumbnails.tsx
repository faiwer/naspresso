import { FolderOpenFilled } from "@ant-design/icons";
import { Link } from "react-router";

import type { StorageDir, StorageEntry } from "../../api/storage.io";
import { routes } from "../../core/routes";
import type { StorageViewProps } from "./StorageList";

import styles from "./StorageThumbnails.module.css";

export const StorageThumbnails = ({ segments, entries }: StorageViewProps) => (
  <div className={styles.grid}>
    {segments.length > 0 && (
      <Thumbnail
        entry={PARENT_ENTRY}
        to={routes.storage(segments.slice(0, -1))}
      />
    )}
    {entries.map((entry) => (
      <Thumbnail
        key={entry.name}
        entry={entry}
        to={
          entry.kind === "dir"
            ? routes.storage([...segments, entry.name])
            : null
        }
      />
    ))}
  </div>
);

type ThumbnailProps = {
  entry: StorageEntry;
  /** Where clicking leads; `null` for files. */
  to: string | null;
};

/** The `..` tile; it isn't a real entry, so its timestamps are dummies. */
const PARENT_ENTRY: StorageDir = {
  kind: "dir",
  name: "..",
  modifiedAt: 0,
  createdAt: null,
};

const Thumbnail = ({ entry, to }: ThumbnailProps) => {
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

  return to == null ? (
    <div className={styles.item}>{content}</div>
  ) : (
    <Link className={styles.item} to={to}>
      {content}
    </Link>
  );
};
