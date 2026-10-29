import { useRef } from "react";

import { isDeepEqual } from "../helpers/object";

/**
 * Returns the previous `value` while the new one is deeply equal to it, so
 * inline objects and arrays can be used as effect deps.
 *
 * @example
 * const stableSrcSet = useMemoValue(srcSet);
 * useLayoutEffect(() => { … }, [stableSrcSet]);
 */
export const useMemoValue = <T>(value: T): T => {
  const ref = useRef(value);
  // Safe: the returned value always equals `value`, only its identity is kept.
  /* eslint-disable react/refs */
  if (!isDeepEqual(ref.current, value)) {
    ref.current = value;
  }
  return ref.current;
  /* eslint-enable react/refs */
};
