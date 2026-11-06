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
import { ThumbnailPreview } from "./ThumbnailPreview";

import styles from "./StorageThumbnails.module.css";

type ThumbnailProps = {
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

export const Thumbnail = ({
  entry,
  segments,
  index,
  openImage,
  encryption,
}: ThumbnailProps) => {
  const { name } = entry;
  const path = [...segments, getOriginalEntryName(entry)];
  const content = (
    <>
      <ThumbnailPreview entry={entry} file={joinPath(path)} index={index} />
      <div className={styles.name} title={name}>
        {name}
      </div>
    </>
  );

  if (entry.kind === "dir") {
    return (
      <Link className={styles.item} to={routes.storage(path)}>
        {content}
      </Link>
    );
  }
  return (
    <StorageFileLink
      className={styles.item}
      href={getEntryFileUrl(segments, entry, encryption === "unlocked")}
      download={entry.name}
      onPreview={isPreviewImage(entry) ? () => openImage(entry) : null}
    >
      {content}
    </StorageFileLink>
  );
};
