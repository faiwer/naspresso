import { Skeleton } from "antd";
import { useMemo } from "react";

import { range } from "../../../core/helpers/array";
import { shortestColumnIndex } from "./packMasonryColumns";

export const useMasonrySkeletons = (
  count: number,
  columnHeights: number[],
  columnWidth: number,
  gap: number,
): number[][] =>
  useMemo(
    () => distributeSkeletons(count, columnHeights, columnWidth, gap),
    [count, columnHeights, columnWidth, gap],
  );

type MasonrySkeletonProps = {
  index: number;
};

export const MasonrySkeleton = ({ index }: MasonrySkeletonProps) => (
  <Skeleton.Node
    active
    styles={{
      root: { display: "block", width: "100%" },
      content: {
        width: "100%",
        height: "auto",
        aspectRatio: `${skeletonAspect(index)}`,
      },
    }}
  />
);

const skeletonAspect = (index: number): number =>
  SKELETON_ASPECTS[index % SKELETON_ASPECTS.length] ?? 1;

const distributeSkeletons = (
  count: number,
  columnHeights: number[],
  columnWidth: number,
  gap: number,
): number[][] => {
  const slots: number[][] = columnHeights.map(() => []);
  const heights = columnHeights.slice();
  for (const index of range(count)) {
    const col = shortestColumnIndex(heights);
    slots[col].push(index);
    heights[col] += columnWidth / skeletonAspect(index) + gap;
  }
  return slots;
};

/** CSS `width / height`, so placeholders aren't all squares. */
const SKELETON_ASPECTS = [4 / 3, 1, 3 / 4, 16 / 9, 5 / 4];
