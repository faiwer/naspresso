import { FolderOpenFilled } from "@ant-design/icons";
import { Link } from "react-router";

import { getFileUrl } from "../../api/storage";
import type { StorageEntry } from "../../api/storage.io";
import { joinPath } from "../../core/helpers/path";
import { routes } from "../../core/routes";
import type { StorageViewProps } from "./StorageList";

import styles from "./StorageThumbnails.module.css";

export const StorageThumbnails = ({ segments, entries }: StorageViewProps) => (
  <div className={styles.grid}>
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
