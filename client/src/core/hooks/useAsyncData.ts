import { useState } from "react";

import { useAsyncEffect } from "./useAsyncEffect";

/** A load is either in flight, settled with data, or failed. */
export type AsyncData<T> =
  | { data: null; isLoading: true; error: null }
  | { data: T; isLoading: false; error: null }
  | { data: null; isLoading: false; error: Error };

/**
 * Runs `fn` whenever `deps` change and reports the load as {@link AsyncData}. Each run
 * resets to loading; a superseded run (aborted signal) writes nothing, so a slow earlier
 * fetch can't overwrite a fresh one. Deps aren't linted — see {@link useAsyncEffect}.
 *
 * @example
 * const listing = useAsyncData((signal) => getListStorage(path, { signal }), [path]);
 */
export const useAsyncData = <T>(
  fn: (signal: AbortSignal) => Promise<T>,
  deps?: unknown[],
): AsyncData<T> => {
  const [state, setState] = useState<AsyncData<T>>(LOADING);

  useAsyncEffect(async (signal) => {
    setState(LOADING);
    try {
      const data = await fn(signal);
      if (!signal.aborted) {
        setState({ data, isLoading: false, error: null });
      }
    } catch (err) {
      if (!signal.aborted) {
        setState({
          data: null,
          isLoading: false,
          error: err instanceof Error ? err : new Error(String(err)),
        });
      }
    }
  }, deps);

  return state;
};

const LOADING: AsyncData<never> = { data: null, isLoading: true, error: null };
