import { useState, useStableCallback, type StateSetter } from "react";
import type { z } from "zod";

import { trackError } from "../errors/trackError";

type StateUpdate<T> = Parameters<StateSetter<T>>[0];

/**
 * `useState` backed by one `localStorage` key. The JSON value is read once, on
 * the first render, and kept only when `schema` accepts it. Otherwise `initial`
 * is used and the stored entry is left as it is. Each update writes
 * `JSON.stringify` of the next value. Changes from other tabs are ignored.
 */
export const useLocalStorageState = <Schema extends z.ZodType>(
  key: string,
  schema: Schema,
  initial: z.infer<Schema> | (() => z.infer<Schema>),
): [z.infer<Schema>, StateSetter<z.infer<Schema>>] => {
  const [state, setState] = useState(() => readStored(key, schema, initial));

  const setStored = useStableCallback(
    (action: StateUpdate<z.infer<Schema>>) => {
      setState((prev) => {
        const next = resolveAction(action, prev);
        writeStored(key, next);
        return next;
      });
    },
  );

  return [state, setStored];
};

const readStored = <Schema extends z.ZodType>(
  key: string,
  schema: Schema,
  initial: z.infer<Schema> | (() => z.infer<Schema>),
): z.infer<Schema> => {
  const fallback = (): z.infer<Schema> =>
    typeof initial === "function"
      ? (initial as () => z.infer<Schema>)()
      : initial;

  try {
    const raw = localStorage.getItem(key);
    if (raw == null) {
      return fallback();
    }
    const parsed = schema.safeParse(JSON.parse(raw));
    return parsed.success ? parsed.data : fallback();
  } catch {
    return fallback();
  }
};

const resolveAction = <T>(action: StateUpdate<T>, prev: T): T =>
  typeof action === "function" ? (action as (value: T) => T)(prev) : action;

const writeStored = (key: string, value: unknown): void => {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch (err) {
    trackError(err, { feature: "unknown", origin: "useLocalStorageState" });
  }
};
