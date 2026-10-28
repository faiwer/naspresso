import { Link } from "react-router";

import { getFileUrl } from "../../api/storage";
import type { StorageEntry } from "../../api/storage.io";
import { joinPath } from "../../core/helpers/path";
import { routes } from "../../core/routes";

export const StorageList = ({ segments, entries }: StorageViewProps) => (
  <ol>
    {entries.map((entry) => (
      <li key={entry.name}>
        {entry.kind === "dir" ? (
          <Link to={routes.storage([...segments, entry.name])}>
            {entry.name}
          </Link>
        ) : (
          <a
            href={getFileUrl(joinPath([...segments, entry.name]))}
            target="_blank"
            rel="noreferrer"
          >
            {entry.name}
          </a>
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
