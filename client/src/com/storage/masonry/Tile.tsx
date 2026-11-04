import { Link } from "react-router";

import { getFileUrl } from "../../../api/storage";
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
  /** The entry's own path. */
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
  const preview = (
    <TilePreview entry={entry} file={joinPath(segments)} index={index} />
  );

  if (entry.kind === "dir") {
    return (
      <Link
        className={styles.item}
        to={routes.storage(segments)}
        title={entry.name}
      >
        {preview}
      </Link>
    );
  }
  return (
    <StorageFileLink
      className={styles.item}
      href={getFileUrl(joinPath(segments), encryption === "unlocked")}
      title={entry.name}
      onPreview={isPreviewImage(entry) ? () => openImage(entry) : null}
    >
      {preview}
    </StorageFileLink>
  );
};
