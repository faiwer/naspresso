import { useRef, useState, useStableCallback } from "react";

import { trackError } from "../errors/trackError";

/**
 * Wraps an async operation with loading and error state. Returns a tuple:
 *
 * - `run` invokes `fn` and resolves with its result. A rejection is stored
 *   in `error` and rethrown, so the caller can `await` it and react too.
 * - `isLoading` is true while the latest `run` is in flight. Overlapping
 *   calls all execute, but only the latest one writes state, so a slow
 *   earlier run can't mark a newer one finished.
 * - `error` is the latest rejection (`fn` may throw anything). Cleared when
 *   a run succeeds, not when the next one starts, so a retry doesn't blink
 *   the previous failure away. Also sent to {@link trackError}.
 *
 * @example
 * const [submit, isSubmitting] = useAsync(async () => {
 *   await createUnlock(path, password);
 * });
 */
export const useAsync = <Args extends unknown[], Ret>(
  fn: (...args: Args) => Promise<Ret>,
): [
  run: (...args: Args) => Promise<Ret>,
  isLoading: boolean,
  error: unknown,
] => {
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<unknown>(null);
  // A run captures its id and writes state only if it is still the latest.
  const latestRunIdRef = useRef(0);

  const run = useStableCallback(async function useAsyncRun(
    ...args: Args
  ): Promise<Ret> {
    latestRunIdRef.current += 1;
    const myRunId = latestRunIdRef.current;
    const isLatest = () => latestRunIdRef.current === myRunId;

    setIsLoading(true);
    try {
      const result = await fn(...args);
      if (isLatest()) {
        setError(null);
        setIsLoading(false);
      }
      return result;
    } catch (err: unknown) {
      if (isLatest()) {
        setError(err);
        setIsLoading(false);
      }
      trackError(err, { feature: "unknown", origin: "useAsync" });
      throw err;
    }
  });

  return [run, isLoading, error];
};
