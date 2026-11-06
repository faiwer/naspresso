import { useRef, type CSSProperties } from "react";

import { getPreviewSrcSet } from "../../../api/media";
import type { ImageMetadata } from "../../../api/storage.io";
import { Image } from "../../image/Image";
import { usePreviewJob } from "../queue/usePreviewJob";

import styles from "./StorageThumbnails.module.css";

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
export const ImagePreview = ({
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
