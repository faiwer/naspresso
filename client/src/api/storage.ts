import { joinPath } from "../core/helpers/path";
import {
  apiGet,
  buildApiUrl,
  encodePath,
  type ApiRequestOptions,
} from "./core";
import type { StorageEntry } from "./storage.io";
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

/** URL of `GET /storage/file/…`: the bytes of a file inside `STORAGE`, e.g. `/a/b/c.jpg`. */
export const getFileUrl = (file: string, unlocked = false): string =>
  buildApiUrl(`/storage/file/${encodePath(file)}`, {
    decode: unlocked || undefined,
  });

/** Name on disk. A decoded file keeps that name in `originalName`. */
export const getOriginalEntryName = (entry: StorageEntry): string =>
  entry.kind === "file" ? (entry.originalName ?? entry.name) : entry.name;

/** {@link getFileUrl} for `entry` inside the directory given as `parentSegments`. */
export const getEntryFileUrl = (
  parentSegments: string[],
  entry: StorageEntry,
  unlocked = false,
): string =>
  getFileUrl(
    joinPath([...parentSegments, getOriginalEntryName(entry)]),
    entry.kind === "file" && !!entry.originalName && unlocked,
  );

type ListStorageOptions = Omit<ApiRequestOptions, "query"> & {
  /** `nextCursor` of the previous page; `null` for the first one. */
  cursor?: string | null;
  /** Page size; the server's default when omitted. */
  limit?: number;
};

export const isEncryptedFolder = (segment: string) => segment.endsWith(".nped");
