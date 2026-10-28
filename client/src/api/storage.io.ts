import { z } from "zod";

/** Fields every entry has. Timestamps are Unix milliseconds. */
const StorageEntryBaseIo = z.object({
  name: z.string(),
  modifiedAt: z.number(),
  /** `null` when the filesystem doesn't record it. */
  createdAt: z.number().nullable(),
});

export const StorageDirIo = StorageEntryBaseIo.extend({
  kind: z.literal("dir"),
});

export const StorageFileIo = StorageEntryBaseIo.extend({
  kind: z.literal("file"),
  /** Bytes. */
  size: z.number(),
});

/** One entry of `GET /storage/ls`. */
export const StorageEntryIo = z.discriminatedUnion("kind", [
  StorageDirIo,
  StorageFileIo,
]);

/** A page of `GET /storage/ls`. */
export const StorageListPageIo = z.object({
  items: z.array(StorageEntryIo),
  /** Pass back as `cursor` for the next page; `null` on the last one. */
  nextCursor: z.string().nullable(),
  /** Entries in the whole directory, across all pages. */
  total: z.number(),
});

export type StorageDir = z.infer<typeof StorageDirIo>;
export type StorageFile = z.infer<typeof StorageFileIo>;
export type StorageEntry = z.infer<typeof StorageEntryIo>;
export type StorageListPage = z.infer<typeof StorageListPageIo>;
