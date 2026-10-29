import { FileFilled, FolderOpenFilled } from "@ant-design/icons";
import { Skeleton } from "antd";
import { useState, type CSSProperties } from "react";
import { Link } from "react-router";

import { getPreviewUrl } from "../../api/media";
import { getFileUrl } from "../../api/storage";
import type { ImageMetadata, StorageEntry } from "../../api/storage.io";
import { range } from "../../core/helpers/array";
import { joinPath } from "../../core/helpers/path";
import { routes } from "../../core/routes";
import type { StorageViewProps } from "./StorageList";

import styles from "./StorageThumbnails.module.css";

export const StorageThumbnails = ({
  segments,
  entries,
  skeletons,
}: StorageViewProps) => (
  <div className={styles.grid}>
    {entries.map((entry) => (
      <Thumbnail
        key={entry.name}
        entry={entry}
        segments={[...segments, entry.name]}
      />
    ))}
    {range(skeletons).map((index) => (
      <ThumbnailSkeleton key={`skeleton-${index}`} />
    ))}
  </div>
);

type ThumbnailProps = {
  entry: StorageEntry;
  /** The entry's own path. */
  segments: string[];
};

const Thumbnail = ({ entry, segments }: ThumbnailProps) => {
  const { name } = entry;
  const content = (
    <>
      <ThumbnailPreview entry={entry} file={joinPath(segments)} />
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
};

/** A folder or file icon, or the image's preview. */
const ThumbnailPreview = ({ entry, file }: ThumbnailPreviewProps) => {
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
      <ImagePreview file={file} image={entry.image} />
    </div>
  );
};

type ImagePreviewProps = {
  file: string;
  image: ImageMetadata;
};

/**
 * The 128px preview, sized from the image's headers until it loads. A box
 * shaped like the image when the server can't make one.
 */
const ImagePreview = ({ file, image }: ImagePreviewProps) => {
  const [failed, setFailed] = useState(false);
  if (failed) {
    return <div className={styles.preview} style={getPreviewStyle(image)} />;
  }

  return (
    <img
      className={styles.image}
      src={getPreviewUrl(file, 128)}
      width={image.width}
      height={image.height}
      alt=""
      loading="lazy"
      decoding="async"
      onError={() => setFailed(true)}
    />
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
