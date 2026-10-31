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

/**
 * How an image carries high dynamic range: `gain_map` is an SDR image plus a
 * gain map (Ultra HDR, ISO 21496-1, Apple); `pq` and `hlg` are HDR transfer
 * functions; `unknown` may be any of these, as headers alone don't tell (a JPEG
 * XL that packs a JPEG).
 */
export const HdrIo = z.enum(["none", "unknown", "gain_map", "pq", "hlg"]);

/** From the image's headers. Pixels, upright: the orientation is applied. */
export const ImageMetadataIo = z.object({
  width: z.number(),
  height: z.number(),
  hdr: HdrIo,
  /** Has frames to play; previews show the first. */
  animated: z.boolean(),
});

export const StorageFileIo = StorageEntryBaseIo.extend({
  kind: z.literal("file"),
  /** Bytes. */
  size: z.number(),
  /** By magic bytes for images, by extension for everything else. */
  mime: z.string(),
  /** `null` for non-images and for images whose headers couldn't be read. */
  image: ImageMetadataIo.nullable(),
  /** Previews of this version are cached, so asking for one won't wait for a render. */
  hasPreview: z.boolean(),
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

export type Hdr = z.infer<typeof HdrIo>;
export type ImageMetadata = z.infer<typeof ImageMetadataIo>;
export type StorageDir = z.infer<typeof StorageDirIo>;
export type StorageFile = z.infer<typeof StorageFileIo>;
export type StorageEntry = z.infer<typeof StorageEntryIo>;
export type StorageListPage = z.infer<typeof StorageListPageIo>;
