import { getPreviewSrcSet } from "../../../api/media";
import type { ImageMetadata } from "../../../api/storage.io";
import { Image } from "../../image/Image";

import styles from "./StorageMasonry.module.css";

type ImagePreviewProps = {
  file: string;
  image: ImageMetadata;
  alt: string;
};

export const ImagePreview = ({ file, image, alt }: ImagePreviewProps) => (
  <div
    className={styles.preview}
    style={{ aspectRatio: `${image.width} / ${image.height}` }}
  >
    <div className={styles.previewFill}>
      <Image
        width={image.width}
        height={image.height}
        srcSet={getPreviewSrcSet(file)}
        loading="lazy"
        alt={alt}
      />
    </div>
  </div>
);
