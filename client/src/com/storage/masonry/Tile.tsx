import { Link } from "react-router";

import { getEntryFileUrl, getOriginalEntryName } from "../../../api/storage";
import type {
  Encryption,
  StorageEntry,
  StorageFile,
} from "../../../api/storage.io";
import { joinPath } from "../../../core/helpers/path";
import { routes } from "../../../core/routes";
import { StorageFileLink } from "../StorageFileLink";
import { isPreviewImage } from "../useOpenStorageImage";
import { TilePreview } from "./TilePreview";

import styles from "./StorageMasonry.module.css";

type TileProps = {
  entry: StorageEntry;
  /** Directory that contains `entry`. */
  segments: string[];
  /** Position in the feed. */
  index: number;
  /** Opens an image in the preview modal. */
  openImage: (file: StorageFile) => void;
  /** The encryption status of the directory. */
  encryption: Encryption;
};

export const Tile = ({
  entry,
  segments,
  index,
  openImage,
  encryption,
}: TileProps) => {
  const path = [...segments, getOriginalEntryName(entry)];
  const preview = (
    <TilePreview entry={entry} file={joinPath(path)} index={index} />
  );

  if (entry.kind === "dir") {
    return (
      <Link
        className={styles.item}
        to={routes.storage(path)}
        title={entry.name}
      >
        {preview}
      </Link>
    );
  }
  return (
    <StorageFileLink
      className={styles.item}
      href={getEntryFileUrl(segments, entry, encryption === "unlocked")}
      download={entry.name}
      title={entry.name}
      onPreview={isPreviewImage(entry) ? () => openImage(entry) : null}
    >
      {preview}
    </StorageFileLink>
  );
};
