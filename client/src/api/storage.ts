import { joinPath } from "../core/helpers/path";
import { apiGet, buildApiUrl, type ApiRequestOptions } from "./core";
import { StorageListPageIo, type StorageListPage } from "./storage.io";

/**
 * `GET /storage/ls?path=…&cursor=…&limit=…`: a page of a directory inside `STORAGE`,
 * directories first, then by name ignoring case.
 */
export const getListStorage = (
  path: string,
  { cursor, limit, ...options }: ListStorageOptions = {},
): Promise<StorageListPage> =>
  apiGet("/storage/ls", StorageListPageIo, {
    ...options,
    query: { path, cursor: cursor ?? undefined, limit },
  });

/** URL of `GET /storage/file?file=…`: the bytes of a file inside `STORAGE`, e.g. `/a/b/c.jpg`. */
export const getFileUrl = (file: string): string =>
  buildApiUrl("/storage/file", { file });

/** {@link getFileUrl} for `fileName` inside the directory given as `parentSegments`. */
export const getEntryFileUrl = (
  parentSegments: string[],
  fileName: string,
): string => getFileUrl(joinPath([...parentSegments, fileName]));

type ListStorageOptions = Omit<ApiRequestOptions, "query"> & {
  /** `nextCursor` of the previous page; `null` for the first one. */
  cursor?: string | null;
  /** Page size; the server's default when omitted. */
  limit?: number;
};
