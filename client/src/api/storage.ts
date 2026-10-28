import { apiGet, type ApiRequestOptions } from "./core";
import { StorageEntriesIo, type StorageEntry } from "./storage.io";

/** `GET /storage/ls?path=…`: entries of a directory inside `STORAGE`, directories first. */
export const getListStorage = (
  path: string,
  options: Omit<ApiRequestOptions, "query"> = {},
): Promise<StorageEntry[]> =>
  apiGet("/storage/ls", StorageEntriesIo, { ...options, query: { path } });
