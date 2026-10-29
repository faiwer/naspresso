import { Link } from "react-router";

import { getFileUrl } from "../../../api/storage";
import type { StorageEntry } from "../../../api/storage.io";
import { joinPath } from "../../../core/helpers/path";
import { routes } from "../../../core/routes";
import { TilePreview } from "./TilePreview";

import styles from "./StorageMasonry.module.css";

type TileProps = {
  entry: StorageEntry;
  /** The entry's own path. */
  segments: string[];
};

export const Tile = ({ entry, segments }: TileProps) => {
  const preview = <TilePreview entry={entry} file={joinPath(segments)} />;

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
    <a
      className={styles.item}
      href={getFileUrl(joinPath(segments))}
      target="_blank"
      rel="noreferrer"
      title={entry.name}
    >
      {preview}
    </a>
  );
};
