import { Skeleton } from "antd";
import { Link } from "react-router";

import { getEntryFileUrl } from "../../api/storage";
import type { StorageEntry } from "../../api/storage.io";
import { range } from "../../core/helpers/array";
import { routes } from "../../core/routes";

export const StorageList = ({
  segments,
  entries,
  skeletons,
}: StorageViewProps) => (
  <ol>
    {entries.map((entry) => (
      <li key={entry.name}>
        {entry.kind === "dir" ? (
          <Link to={routes.storage([...segments, entry.name])}>
            {entry.name}
          </Link>
        ) : (
          <a
            href={getEntryFileUrl(segments, entry.name)}
            target="_blank"
            rel="noreferrer"
          >
            {entry.name}
          </a>
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
};
