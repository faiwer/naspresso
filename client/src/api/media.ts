import type { ImageSource } from "../com/image/Image";
import { buildApiUrl } from "./core";

/**
 * URL of `GET /media/preview/{size}/…`: an AVIF of an image inside `STORAGE`,
 * e.g. `/a/b/c.jpg`, whose longest side is at most `size`. Never upscaled.
 */
export const getPreviewUrl = (file: string, size: PreviewSize): string =>
  buildApiUrl(`/media/preview/${size}${encodePath(file)}`);

export type PreviewSize = 128 | 256 | 512;

/** Every preview of an image, as `<Image srcSet>`. */
export const getPreviewSrcSet = (file: string): ImageSource[] =>
  PREVIEW_SIZES.map((size) => ({ size, url: getPreviewUrl(file, size) }));

const PREVIEW_SIZES: PreviewSize[] = [128, 256, 512];

/** Encodes each segment, keeping the slashes between them. */
const encodePath = (path: string): string =>
  path.split("/").map(encodeURIComponent).join("/");
