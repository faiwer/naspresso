import clsx from "clsx";

import type { StorageEntry } from "../../../api/storage.io";
import { getExtension } from "../../../core/helpers/path";
import { FileIcon } from "../../fileIcon/FileIcon";
import { FolderIcon } from "../../FolderIcon/FolderIcon";
import { ImagePreview } from "./ImagePreview";

import styles from "./StorageThumbnails.module.css";

type ThumbnailPreviewProps = {
  entry: StorageEntry;
  /** The entry's path, like `/a/b/c.jpg`. */
  file: string;
  /** Position in the feed. */
  index: number;
};

/** A folder or file icon, or the image's preview. */
export const ThumbnailPreview = ({
  entry,
  file,
  index,
}: ThumbnailPreviewProps) => {
  if (entry.kind === "dir") {
    return (
      <div className={clsx(styles.icon, styles.folder)}>
        <FolderIcon name={entry.name} />
      </div>
    );
  }
  if (!entry.image) {
    return (
      <div className={clsx(styles.icon, styles.file)}>
        <FileIcon mime={entry.mime} ext={getExtension(entry.name)} />
      </div>
    );
  }
  return (
    <div className={styles.frame}>
      <ImagePreview
        file={file}
        image={entry.image}
        index={index}
        hasPreview={entry.hasPreview}
      />
    </div>
  );
};
