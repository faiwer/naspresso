import clsx from "clsx";
import { useLayoutEffect, useRef, useState } from "react";

import { trackError } from "../../core/errors/trackError";
import { useMemoValue } from "../../core/hooks/useMemoValue";
import { pickSource } from "./pickSource";
import { SrcImage } from "./SrcImage";

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
  /** When false, the <div/> stays and no request is made. */
  enabled?: boolean;
  /**
   * On unmount, remove the `src` attribute so Chrome and Firefox abort the
   * HTTP request. `src = ""` aborts them too, but an empty source fires
   * `error`. Removing the attribute does not.
   *
   * `remove()` alone does not abort. Chrome, Firefox, and Safari keep
   * reading a disconnected `<img>` to the end. Clearing `src` still aborts
   * Chrome and Firefox after the node is disconnected. Safari finishes the
   * body either way, and then fires neither `load` nor `error`.
   *
   * The abort matters because the server queues preview creation. If that
   * work has not started, the disconnect can cancel it, so the server
   * spends its slots on the previews still on screen.
   */
  clearSrcOnDestroy?: boolean;
  alt?: string;
  className?: string;
  onLoad?: () => void;
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
  enabled = true,
  clearSrcOnDestroy = true,
  alt = "",
  className,
  onLoad,
  onError,
}: ImageProps) => {
  const stableSrcSet = useMemoValue(srcSet);
  const rootRef = useRef<HTMLDivElement>(null);
  const [src, setSrc] = useState<string | null>(null);
  const [ready, setReady] = useState<{
    ready: boolean;
    src: string;
    error: boolean;
  } | null>(null);

  useLayoutEffect(() => {
    const root = rootRef.current;
    if (!enabled || !root) {
      return;
    }

    setReady(null);
    setSrc(
      pickSource(
        stableSrcSet,
        { width, height },
        { width: root.clientWidth, height: root.clientHeight },
        window.devicePixelRatio,
      ),
    );
  }, [enabled, stableSrcSet, width, height]);

  const outcome = enabled && !!src && ready?.src === src ? ready : null;

  return (
    <div
      ref={rootRef}
      className={clsx(
        styles.root,
        !outcome && styles.pending,
        outcome?.error && styles.withError,
        className,
      )}
    >
      {enabled && src && !outcome?.error && (
        <SrcImage
          key={src}
          src={src}
          alt={alt}
          loading={loading}
          clearSrcOnDestroy={clearSrcOnDestroy}
          className={clsx(styles.image, outcome && styles.ready)}
          onLoad={(image) => {
            onLoad?.();
            // A failed decode() still leaves a loaded, paintable image.
            void image
              .decode()
              .catch((err: unknown) => {
                trackError(err, {
                  feature: "image",
                  origin: "Image.decode",
                  extra: { src },
                });
              })
              .then(() => setReady({ ready: true, src, error: false }));
          }}
          onError={() => {
            onError?.();
            setReady({ ready: false, src, error: true });
          }}
        />
      )}
      {outcome?.error && (
        <img className={styles.error} src="/error.svg" alt="" />
      )}
    </div>
  );
};
