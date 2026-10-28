import { useState, useStableCallback } from "react";

import { getListStorage } from "../../api/storage";
import type { StorageEntry } from "../../api/storage.io";
import { useAsyncEffect } from "../../core/hooks/useAsyncEffect";

/** Small on purpose, to exercise paging; the server's default is 30. */
export const STORAGE_PAGE_SIZE = 30;

export type StorageFeed = {
  /** Every page loaded so far, in listing order. */
  items: StorageEntry[];
  /** Entries in the whole directory; `null` until the first page arrives. */
  total: number | null;
  isLoading: boolean;
  /** Why the last page failed; cleared by {@link StorageFeed.retry}. */
  error: Error | null;
  /** A next page exists and nothing is in flight or failed. */
  canLoadMore: boolean;
  loadMore: () => void;
  /** Requests the failed page again. */
  retry: () => void;
};

/**
 * Pages through the directory at `path`: the first page on mount, each next one
 * on {@link StorageFeed.loadMore}. It lists one path for its whole life:
 * remount the component (`key={path}`) to list another.
 */
export const useStorageFeed = (path: string): StorageFeed => {
  const [state, setState] = useState<FeedState>(INITIAL_STATE);
  // Bumped by `retry` to re-run the fetch effect for the same cursor.
  const [attempt, setAttempt] = useState(0);

  useAsyncEffect(
    async (signal) => {
      setState((prev) => ({ ...prev, isLoading: true, error: null }));
      try {
        const page = await getListStorage(path, {
          cursor: state.cursor,
          limit: STORAGE_PAGE_SIZE,
          signal,
        });
        if (!signal.aborted) {
          setState((prev) => ({
            ...prev,
            items: [...prev.items, ...page.items],
            total: page.total,
            nextCursor: page.nextCursor,
            isLoading: false,
          }));
        }
      } catch (err) {
        if (!signal.aborted) {
          setState((prev) => ({
            ...prev,
            isLoading: false,
            error: err instanceof Error ? err : new Error(String(err)),
          }));
        }
      }
    },
    [state.cursor, attempt],
  );

  const canLoadMore =
    state.nextCursor != null && !state.isLoading && state.error == null;

  const loadMore = useStableCallback(() => {
    if (canLoadMore) {
      setState((prev) => ({ ...prev, cursor: prev.nextCursor }));
    }
  });

  const retry = useStableCallback(() => setAttempt((prev) => prev + 1));

  return {
    items: state.items,
    total: state.total,
    isLoading: state.isLoading,
    error: state.error,
    canLoadMore,
    loadMore,
    retry,
  };
};

type FeedState = {
  items: StorageEntry[];
  total: number | null;
  /** The page being fetched or last fetched; `null` for the first one. */
  cursor: string | null;
  /** The page after `items`; `null` on the last page and before the first one arrives. */
  nextCursor: string | null;
  isLoading: boolean;
  error: Error | null;
};

const INITIAL_STATE: FeedState = {
  items: [],
  total: null,
  cursor: null,
  nextCursor: null,
  isLoading: true,
  error: null,
};
