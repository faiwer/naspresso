import { useRef } from "react";

import { getPreviewSrcSet } from "../../../api/media";
import type { ImageMetadata } from "../../../api/storage.io";
import { Image } from "../../image/Image";
import { usePreviewJob } from "../queue/usePreviewJob";

import styles from "./StorageMasonry.module.css";

type ImagePreviewProps = {
  file: string;
  image: ImageMetadata;
  alt: string;
  /** Position in the feed. */
  index: number;
  hasPreview: boolean;
};

export const ImagePreview = ({
  file,
  image,
  alt,
  index,
  hasPreview,
}: ImagePreviewProps) => {
  const ref = useRef<HTMLDivElement>(null);
  const preview = usePreviewJob(ref, file, index, hasPreview);
  return (
    <div
      ref={ref}
      className={styles.preview}
      style={{ aspectRatio: `${image.width} / ${image.height}` }}
    >
      <div className={styles.previewFill}>
        <Image
          width={image.width}
          height={image.height}
          srcSet={getPreviewSrcSet(file)}
          loading="eager"
          alt={alt}
          enabled={preview.enabled}
          onLoad={preview.onLoad}
          onError={preview.onError}
        />
      </div>
    </div>
  );
};
