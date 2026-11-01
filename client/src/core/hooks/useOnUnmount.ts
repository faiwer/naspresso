import { useEffect, useStableCallback } from "react";

/** Calls `onUnmount` once, when the component unmounts. */
export const useOnUnmount = (onUnmount: () => void): void => {
  const onUnmountStable = useStableCallback(onUnmount);

  useEffect(() => onUnmountStable, [onUnmountStable]);
};
