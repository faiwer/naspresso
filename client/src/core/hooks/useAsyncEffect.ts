import { useEffect } from "react";
import { trackError } from "../errors/trackError";

type AsyncEffectCallback = (signal: AbortSignal) => Promise<void> | (() => void) | void;

/**
 * `useEffect` for async work: the callback may be `async`, gets an `AbortSignal` that's
 * aborted on teardown, and its rejections go to {@link trackError} (aborts excluded).
 * A sync callback may still return a cleanup function.
 *
 * Deps aren't checked by `exhaustive-deps`: list what should re-run the effect.
 *
 * @example
 * useAsyncEffect(async (signal) => {
 *   const data = await getThing(id, { signal });
 *   if (!signal.aborted) {
 *     setData(data);
 *   }
 * }, [id]);
 */
export const useAsyncEffect = (fn: AsyncEffectCallback, deps?: unknown[]): void => {
  useEffect(() => {
    const controller = new AbortController();
    const result = fn(controller.signal);

    // Duck-typed like React does, so promise-likes from other realms count too.
    if (result && typeof result === "object" && "then" in result) {
      result.catch((err: unknown) => {
        if (err instanceof DOMException && err.name === "AbortError") {
          return;
        }
        trackError(err, { feature: "unknown", origin: "useAsyncEffect" });
      });
    }

    return () => {
      controller.abort();
      if (typeof result === "function") {
        result();
      }
    };
    // eslint-disable-next-line react/exhaustive-deps
  }, deps);
};
