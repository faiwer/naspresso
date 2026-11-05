import { Skeleton } from "antd";
import { useRef, type CSSProperties } from "react";
import { Link } from "react-router";

import { getPreviewSrcSet } from "../../api/media";
import { getEntryFileUrl, getOriginalEntryName } from "../../api/storage";
import type {
  ImageMetadata,
  Encryption,
  StorageEntry,
  StorageFile,
} from "../../api/storage.io";
import { range } from "../../core/helpers/array";
import { getExtension, joinPath } from "../../core/helpers/path";
import { routes } from "../../core/routes";
import { FileIcon } from "../fileIcon/FileIcon";
import { FolderIcon } from "../FolderIcon/FolderIcon";
import { Image } from "../image/Image";
import { usePreviewJob } from "./queue/usePreviewJob";
import { PreviewQueueProvider, usePreviewQueue } from "./queue/usePreviewQueue";
import { StorageFileLink } from "./StorageFileLink";
import type { StorageViewProps } from "./StorageList";
import { isPreviewImage } from "./useOpenStorageImage";

import styles from "./StorageThumbnails.module.css";

export const StorageThumbnails = ({
  segments,
  entries,
  skeletons,
  scrollContainer,
  openImage,
  encryption,
}: StorageViewProps) => {
  const queue = usePreviewQueue(scrollContainer);
  return (
    <PreviewQueueProvider queue={queue}>
      <div className={styles.grid}>
        {entries.map((entry, index) => (
          <Thumbnail
            key={getOriginalEntryName(entry)}
            entry={entry}
            index={index}
            segments={segments}
            openImage={openImage}
            encryption={encryption}
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
  /** Directory that contains `entry`. */
  segments: string[];
  /** Position in the feed. */
  index: number;
  /** Opens an image in the preview modal. */
  openImage: (file: StorageFile) => void;
  /** The encryption status of the directory. */
  encryption: Encryption;
};

const Thumbnail = ({
  entry,
  segments,
  index,
  openImage,
  encryption,
}: ThumbnailProps) => {
  const { name } = entry;
  const path = [...segments, getOriginalEntryName(entry)];
  const content = (
    <>
      <ThumbnailPreview entry={entry} file={joinPath(path)} index={index} />
      <div className={styles.name} title={name}>
        {name}
      </div>
    </>
  );

  if (entry.kind === "dir") {
    return (
      <Link className={styles.item} to={routes.storage(path)}>
        {content}
      </Link>
    );
  }
  return (
    <StorageFileLink
      className={styles.item}
      href={getEntryFileUrl(segments, entry, encryption === "unlocked")}
      download={entry.name}
      onPreview={isPreviewImage(entry) ? () => openImage(entry) : null}
    >
      {content}
    </StorageFileLink>
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
        <FolderIcon name={entry.name} />
      </div>
    );
  }
  if (!entry.image) {
    return (
      <div className={`${styles.icon} ${styles.file}`}>
        <FileIcon mime={entry.mime} ext={getExtension(entry.name)} />
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
