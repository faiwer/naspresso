import { Link, useLocation } from "react-router";
import { getListStorage } from "../api/storage";
import { useAsyncData } from "../core/hooks/useAsyncData";
import { parseStorageUrl, routes } from "../core/routes";

/**
 * Lists a directory of `STORAGE`. Mounted at `/storage/*`; the rest of the URL is the
 * directory path, built by `routes.storage`.
 */
export const StoragePage = () => {
  const segments = parseStorageUrl(useLocation().pathname);
  const path = "/" + segments.join("/");
  const listing = useAsyncData((signal) => getListStorage(path, { signal }), [path]);

  if (listing.isLoading) {
    return <p>…</p>;
  }
  if (listing.error) {
    return <p>{listing.error.message}</p>;
  }

  return (
    <ol>
      {segments.length > 0 && (
        <li>
          <Link to={routes.storage(segments.slice(0, -1))}>..</Link>
        </li>
      )}
      {listing.data.map((entry) => (
        <li key={entry.name}>
          {entry.kind === "dir" ? (
            <Link to={routes.storage([...segments, entry.name])}>{entry.name}</Link>
          ) : (
            entry.name
          )}
        </li>
      ))}
    </ol>
  );
};
