import { useEffect, useRef, useStableCallback } from "react";

type ScrollSentinelProps = {
  /** The scrolling element the sentinel is rendered in. */
  root: HTMLElement;
  onReached: () => void;
};

/**
 * An invisible marker that calls `onReached` once it's within 150% of `root`'s
 * visible height from its top. An observer only reports changes, so a sentinel
 * that stays in range doesn't fire again: remount it to check afresh.
 */
export const ScrollSentinel = ({ root, onReached }: ScrollSentinelProps) => {
  const ref = useRef<HTMLDivElement>(null);
  const onReachedStable = useStableCallback(onReached);

  useEffect(() => {
    const node = ref.current;
    if (!node) {
      return;
    }

    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) {
          onReachedStable();
        }
      },
      { root, rootMargin: ROOT_MARGIN },
    );
    observer.observe(node);
    return () => observer.disconnect();
  }, [root, onReachedStable]);

  return <div ref={ref} />;
};

/** Grows `root`'s box downwards by half its height. */
const ROOT_MARGIN = "0px 0px 50% 0px";
