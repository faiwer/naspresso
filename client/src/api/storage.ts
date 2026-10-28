import { joinPath } from "../core/helpers/path";
import { apiGet, buildApiUrl, type ApiRequestOptions } from "./core";
import { StorageEntriesIo, type StorageEntry } from "./storage.io";

/** `GET /storage/ls?path=…`: entries of a directory inside `STORAGE`, directories first. */
export const getListStorage = (
  path: string,
  options: Omit<ApiRequestOptions, "query"> = {},
): Promise<StorageEntry[]> =>
  apiGet("/storage/ls", StorageEntriesIo, { ...options, query: { path } });

/** URL of `GET /storage/file?file=…`: the bytes of a file inside `STORAGE`, e.g. `/a/b/c.jpg`. */
export const getFileUrl = (file: string): string =>
  buildApiUrl("/storage/file", { file });

/** {@link getFileUrl} for `fileName` inside the directory given as `parentSegments`. */
export const getEntryFileUrl = (
  parentSegments: string[],
  fileName: string,
): string => getFileUrl(joinPath([...parentSegments, fileName]));
