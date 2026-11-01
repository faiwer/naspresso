import type { StorageFile } from "../../../api/storage.io";
import { nullthrows } from "../../../core/helpers/nullthrows";
import { fittedSize } from "../fittedSize";
import type { FrameSide } from "../frameEnabled";

export type Viewport = { width: number; height: number };

/** Fitted width of a preview image. Every file in the session has one. */
export const frameWidth = (file: StorageFile, viewport: Viewport): number =>
  fittedSize(nullthrows(file.image), viewport).width;

/** The neighbor a horizontal drag moves toward, when that image exists. */
export const neighborAt = (
  files: StorageFile[],
  pos: number,
  dx: number,
  viewport: Viewport,
): { side: FrameSide; width: number } | null => {
  if (dx === 0) {
    // No dx = no direction = no neighbor.
    return null;
  }

  const side = dx > 0 ? "left" : "right";
  const file = files[dx > 0 ? pos - 1 : pos + 1] ?? null;
  if (file == null) {
    return null;
  }

  return { side, width: frameWidth(file, viewport) };
};
