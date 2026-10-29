import { FileFilled, FolderOpenFilled } from "@ant-design/icons";
import clsx from "clsx";

import type { StorageEntry } from "../../../api/storage.io";
import { ImagePreview } from "./ImagePreview";

import styles from "./StorageMasonry.module.css";

type TilePreviewProps = {
  entry: StorageEntry;
  /** The entry's path, like `/a/b/c.jpg`. */
  file: string;
};

export const TilePreview = ({ entry, file }: TilePreviewProps) => {
  if (entry.kind === "dir") {
    return (
      <div className={clsx(styles.icon, styles.folder)}>
        <FolderOpenFilled />
      </div>
    );
  }
  if (!entry.image) {
    return (
      <div className={clsx(styles.icon, styles.file)}>
        <FileFilled />
      </div>
    );
  }
  return <ImagePreview file={file} image={entry.image} alt={entry.name} />;
};
