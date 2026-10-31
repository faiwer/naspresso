import { useLayoutEffect, useRef } from "react";

import { nullthrows } from "../../core/helpers/nullthrows";

type SrcImageProps = {
  src: string;
  alt: string;
  loading?: "lazy" | "eager";
  className?: string;
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
  clearSrcOnDestroy: boolean;
  onLoad: (image: HTMLImageElement) => void;
  onError: () => void;
};

/** The `<img>`. Unmount aborts its request by clearing `src`. */
export const SrcImage = ({
  src,
  alt,
  loading,
  className,
  clearSrcOnDestroy,
  onLoad,
  onError,
}: SrcImageProps) => {
  const ref = useRef<HTMLImageElement>(null);
  const clearSrcOnDestroyRef = useRef(clearSrcOnDestroy);

  useLayoutEffect(() => {
    clearSrcOnDestroyRef.current = clearSrcOnDestroy;
  }, [clearSrcOnDestroy]);

  useLayoutEffect(() => {
    const img = nullthrows(ref.current);
    return () => {
      if (clearSrcOnDestroyRef.current) {
        img.removeAttribute("src");
      }
    };
  }, []);

  return (
    <img
      ref={ref}
      className={className}
      src={src}
      alt={alt}
      loading={loading}
      decoding="async"
      onLoad={(event) => {
        onLoad(event.currentTarget as HTMLImageElement);
      }}
      onError={onError}
    />
  );
};
