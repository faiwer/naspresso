import { Skeleton } from "antd";
import { Link } from "react-router";

import { getEntryFileUrl } from "../../api/storage";
import type { StorageEntry, StorageFile } from "../../api/storage.io";
import { range } from "../../core/helpers/array";
import { routes } from "../../core/routes";
import { StorageFileLink } from "./StorageFileLink";
import { isPreviewImage } from "./useOpenStorageImage";

export const StorageList = ({
  segments,
  entries,
  skeletons,
  openImage,
}: StorageViewProps) => (
  <ol>
    {entries.map((entry) => (
      <li key={entry.name}>
        {entry.kind === "dir" ? (
          <Link to={routes.storage([...segments, entry.name])}>
            {entry.name}
          </Link>
        ) : (
          <StorageFileLink
            href={getEntryFileUrl(segments, entry.name)}
            onPreview={isPreviewImage(entry) ? () => openImage(entry) : null}
          >
            {entry.name}
          </StorageFileLink>
        )}
      </li>
    ))}
    {range(skeletons).map((index) => (
      <li key={`skeleton-${index}`}>
        <Skeleton.Input active size="small" />
      </li>
    ))}
  </ol>
);

/** Props shared by every storage view mode. */
export type StorageViewProps = {
  /** The listed directory as path segments; empty for the root. */
  segments: string[];
  entries: StorageEntry[];
  /** Placeholders to show after `entries` while more of them load. */
  skeletons: number;
  /** The element the listing scrolls in. Preview modes observe it. */
  scrollContainer: HTMLElement;
  /** Opens an image in the preview modal, among the images in `entries`. */
  openImage: (file: StorageFile) => void;
};
