import { FileFilled, FolderOpenFilled } from "@ant-design/icons";
import { Skeleton } from "antd";
import { useRef, type CSSProperties } from "react";
import { Link } from "react-router";

import { getPreviewSrcSet } from "../../api/media";
import { getFileUrl } from "../../api/storage";
import type { ImageMetadata, StorageEntry } from "../../api/storage.io";
import { range } from "../../core/helpers/array";
import { joinPath } from "../../core/helpers/path";
import { routes } from "../../core/routes";
import { Image } from "../image/Image";
import { usePreviewJob } from "./queue/usePreviewJob";
import { PreviewQueueProvider, usePreviewQueue } from "./queue/usePreviewQueue";
import type { StorageViewProps } from "./StorageList";

import styles from "./StorageThumbnails.module.css";

export const StorageThumbnails = ({
  segments,
  entries,
  skeletons,
  scrollContainer,
}: StorageViewProps) => {
  const queue = usePreviewQueue(scrollContainer);
  return (
    <PreviewQueueProvider queue={queue}>
      <div className={styles.grid}>
        {entries.map((entry, index) => (
          <Thumbnail
            key={entry.name}
            entry={entry}
            index={index}
            segments={[...segments, entry.name]}
          />
        ))}
        {range(skeletons).map((index) => (
          <ThumbnailSkeleton key={`skeleton-${index}`} />
        ))}
      </div>
    </PreviewQueueProvider>
  );
};

type ThumbnailProps = {
  entry: StorageEntry;
  /** The entry's own path. */
  segments: string[];
  /** Position in the feed. */
  index: number;
};

const Thumbnail = ({ entry, segments, index }: ThumbnailProps) => {
  const { name } = entry;
  const content = (
    <>
      <ThumbnailPreview entry={entry} file={joinPath(segments)} index={index} />
      <div className={styles.name} title={name}>
        {name}
      </div>
    </>
  );

  if (entry.kind === "dir") {
    return (
      <Link className={styles.item} to={routes.storage(segments)}>
        {content}
      </Link>
    );
  }
  return (
    <a
      className={styles.item}
      href={getFileUrl(joinPath(segments))}
      target="_blank"
      rel="noreferrer"
    >
      {content}
    </a>
  );
};

type ThumbnailPreviewProps = {
  entry: StorageEntry;
  /** The entry's path, like `/a/b/c.jpg`. */
  file: string;
  /** Position in the feed. */
  index: number;
};

/** A folder or file icon, or the image's preview. */
const ThumbnailPreview = ({ entry, file, index }: ThumbnailPreviewProps) => {
  if (entry.kind === "dir") {
    return (
      <div className={`${styles.icon} ${styles.folder}`}>
        <FolderOpenFilled />
      </div>
    );
  }
  if (!entry.image) {
    return (
      <div className={`${styles.icon} ${styles.file}`}>
        <FileFilled />
      </div>
    );
  }
  return (
    <div className={styles.frame}>
      <ImagePreview
        file={file}
        image={entry.image}
        index={index}
        hasPreview={entry.hasPreview}
      />
    </div>
  );
};

type ImagePreviewProps = {
  file: string;
  image: ImageMetadata;
  /** Position in the feed. */
  index: number;
  hasPreview: boolean;
};

/**
 * The preview in a box shaped like the image's headers; a placeholder until it
 * loads, or for good when the server can't make one.
 */
const ImagePreview = ({
  file,
  image,
  index,
  hasPreview,
}: ImagePreviewProps) => {
  const ref = useRef<HTMLDivElement>(null);
  const preview = usePreviewJob(ref, file, index, hasPreview);
  return (
    <div ref={ref} className={styles.preview} style={getPreviewStyle(image)}>
      <Image
        width={image.width}
        height={image.height}
        srcSet={getPreviewSrcSet(file)}
        loading="eager"
        enabled={preview.enabled}
        onLoad={preview.onLoad}
        onError={preview.onError}
      />
    </div>
  );
};

const ThumbnailSkeleton = () => (
  <div className={styles.skeleton}>
    <Skeleton.Node active styles={PREVIEW_SKELETON_STYLES} />
    <Skeleton.Input active block size="small" className={styles.skeletonName} />
  </div>
);

/**
 * Fits the preview in its square frame with the image's proportions: full
 * width when landscape, full height when portrait.
 */
const getPreviewStyle = ({ width, height }: ImageMetadata): CSSProperties => {
  const aspectRatio = `${width} / ${height}`;
  return width >= height
    ? { aspectRatio, width: "100%" }
    : { aspectRatio, height: "100%" };
};

/** Stretches the node to the tile's width, square like a preview. */
const PREVIEW_SKELETON_STYLES = {
  root: { display: "block" },
  content: { width: "100%", height: "auto", aspectRatio: "1" },
};
