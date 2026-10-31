import { FileFilled, FolderOpenFilled } from "@ant-design/icons";
import clsx from "clsx";

import type { StorageEntry } from "../../../api/storage.io";
import { ImagePreview } from "./ImagePreview";

import styles from "./StorageMasonry.module.css";

type TilePreviewProps = {
  entry: StorageEntry;
  /** The entry's path, like `/a/b/c.jpg`. */
  file: string;
  /** Position in the feed. */
  index: number;
};

export const TilePreview = ({ entry, file, index }: TilePreviewProps) => {
  if (entry.kind === "dir") {
    return (
      <div className={clsx(styles.icon, styles.folder)}>
        <FolderOpenFilled />
        <span className={styles.name}>{entry.name}</span>
      </div>
    );
  }
  if (!entry.image) {
    return (
      <div className={clsx(styles.icon, styles.file)}>
        <FileFilled />
        <span className={styles.name}>{entry.name}</span>
      </div>
    );
  }
  return (
    <ImagePreview
      file={file}
      image={entry.image}
      alt={entry.name}
      index={index}
      hasPreview={entry.hasPreview}
    />
  );
};
