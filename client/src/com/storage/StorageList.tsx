import { Link } from "react-router";

import type { StorageEntry } from "../../api/storage.io";
import { routes } from "../../core/routes";

export const StorageList = ({ segments, entries }: StorageViewProps) => (
  <ol>
    {segments.length > 0 && (
      <li>
        <Link to={routes.storage(segments.slice(0, -1))}>..</Link>
      </li>
    )}
    {entries.map((entry) => (
      <li key={entry.name}>
        {entry.kind === "dir" ? (
          <Link to={routes.storage([...segments, entry.name])}>
            {entry.name}
          </Link>
        ) : (
          entry.name
        )}
      </li>
    ))}
  </ol>
);

/** Props shared by every storage view mode. */
export type StorageViewProps = {
  /** The listed directory as path segments; empty for the root. */
  segments: string[];
  entries: StorageEntry[];
};
