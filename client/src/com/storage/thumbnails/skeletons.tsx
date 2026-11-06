import { Skeleton } from "antd";

import styles from "./StorageThumbnails.module.css";

export const ThumbnailSkeleton = () => (
  <div className={styles.skeleton}>
    <Skeleton.Node active styles={PREVIEW_SKELETON_STYLES} />
    <Skeleton.Input active block size="small" className={styles.skeletonName} />
  </div>
);

/** Stretches the node to the tile's width, square like a preview. */
const PREVIEW_SKELETON_STYLES = {
  root: { display: "block" },
  content: { width: "100%", height: "auto", aspectRatio: "1" },
};
