import clsx from "clsx";
import { useLayoutEffect, useRef, useState } from "react";

import { trackError } from "../../core/errors/trackError";
import { useMemoValue } from "../../core/hooks/useMemoValue";
import { pickSource } from "./pickSource";

import styles from "./Image.module.css";

export type ImageSource = {
  /** The variant's longest side is at most this, never upscaled. */
  size: number;
  url: string;
};

type ImageProps = {
  /** The original's dimensions. */
  width: number;
  height: number;
  /** The same image in different sizes. */
  srcSet: ImageSource[];
  loading?: "lazy" | "eager";
  alt?: string;
  className?: string;
  onError?: () => void;
};

/**
 * Fills its box, which the owner sizes with the image's aspect ratio. Measures
 * the box, picks the variant that fits the screen's pixel density, and shows a
 * placeholder until that variant is loaded and decoded.
 */
export const Image = ({
  width,
  height,
  srcSet,
  loading,
  alt = "",
  className,
  onError,
}: ImageProps) => {
  const stableSrcSet = useMemoValue(srcSet);
  const rootRef = useRef<HTMLDivElement>(null);
  const [src, setSrc] = useState<string | null>(null);
  const [readySrc, setReadySrc] = useState<string | null>(null);

  useLayoutEffect(() => {
    const root = rootRef.current;
    if (!root) {
      return;
    }

    setReadySrc(null);
    setSrc(
      pickSource(
        stableSrcSet,
        { width, height },
        { width: root.clientWidth, height: root.clientHeight },
        window.devicePixelRatio,
      ),
    );
  }, [stableSrcSet, width, height]);

  const ready = !!src && readySrc === src;

  return (
    <div
      ref={rootRef}
      className={clsx(styles.root, !ready && styles.pending, className)}
    >
      {src && (
        <img
          key={src}
          className={clsx(styles.image, ready && styles.ready)}
          src={src}
          alt={alt}
          loading={loading}
          decoding="async"
          onLoad={(event) => {
            // A failed decode() still leaves a loaded, paintable image.
            void (event.currentTarget as HTMLImageElement)
              .decode()
              .catch((err: unknown) => {
                trackError(err, {
                  feature: "image",
                  origin: "Image.decode",
                  extra: { src },
                });
              })
              .then(() => setReadySrc(src));
          }}
          onError={onError}
        />
      )}
    </div>
  );
};
