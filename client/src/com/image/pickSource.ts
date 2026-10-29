import type { ImageSource } from "./Image";

export type Dimensions = { width: number; height: number };

/**
 * The smallest variant covering `box` in device pixels, give or take
 * {@link UPSCALE_TOLERANCE}; the largest one when none does.
 */
export const pickSource = (
  srcSet: ImageSource[],
  original: Dimensions,
  box: Dimensions,
  pixelRatio: number,
): string | null => {
  const longest = Math.max(original.width, original.height);
  const scale = Math.max(
    box.width / original.width,
    box.height / original.height,
  );

  const needed = scale * longest * pixelRatio;
  let largest: { side: number; url: string } | null = null;

  for (const { size, url } of srcSet.toSorted((a, b) => a.size - b.size)) {
    const side = Math.min(size, longest);

    if (side * UPSCALE_TOLERANCE >= needed) {
      return url;
    }

    if (!largest || side > largest.side) {
      largest = { side, url };
    }
  }

  return largest?.url ?? null;
};

/** A variant this much smaller than needed still beats a 4× heavier one. */
const UPSCALE_TOLERANCE = 1.1;
