import { createRoot, useLayoutEffect, type StateSetter } from "react";
import { act } from "react/testing";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { z } from "zod";

import { useLocalStorageState } from "./useLocalStorageState";

const Mode = z.enum(["list", "masonry", "table", "thumbnails"]);
type Mode = z.infer<typeof Mode>;

const KEY = "storage.viewMode";

let cleanup: (() => Promise<void>) | null = null;
const api: { setMode: StateSetter<Mode> | null } = {
  setMode: null,
};

const Probe = ({
  initial = "thumbnails",
}: {
  initial?: Mode | (() => Mode);
}) => {
  const [mode, setMode] = useLocalStorageState(KEY, Mode, initial);
  useLayoutEffect(() => {
    api.setMode = setMode;
  }, [setMode]);
  return <span>{mode}</span>;
};

const mount = async (
  initial?: Mode | (() => Mode),
): Promise<{ host: HTMLDivElement }> => {
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  await act(() => {
    root.render(<Probe initial={initial} />);
  });
  cleanup = async () => {
    await act(() => {
      root.unmount();
    });
    host.remove();
  };
  return { host };
};

const setMode = (action: Parameters<StateSetter<Mode>>[0]): Promise<void> => {
  const set = api.setMode;
  if (set == null) {
    throw new Error("setter is not ready");
  }
  return act(() => {
    set(action);
  });
};

beforeEach(() => {
  localStorage.clear();
  api.setMode = null;
});

afterEach(async () => {
  await cleanup?.();
  cleanup = null;
});

describe("useLocalStorageState", () => {
  it("uses the initial value when the key is missing and does not write it", async () => {
    const initial = vi.fn(() => "list" as const);
    const { host } = await mount(initial);

    expect(host.textContent).toBe("list");
    expect(initial).toHaveBeenCalledOnce();
    expect(localStorage.getItem(KEY)).toBeNull();
  });

  it("uses a stored value that passes the schema and skips the initial value", async () => {
    localStorage.setItem(KEY, JSON.stringify("masonry"));
    const initial = vi.fn(() => "list" as const);
    const { host } = await mount(initial);

    expect(host.textContent).toBe("masonry");
    expect(initial).not.toHaveBeenCalled();
  });

  it("falls back when the stored json fails the schema", async () => {
    localStorage.setItem(KEY, JSON.stringify("grid"));
    const { host } = await mount();

    expect(host.textContent).toBe("thumbnails");
    expect(localStorage.getItem(KEY)).toBe(JSON.stringify("grid"));
  });

  it("falls back when the stored value is not json", async () => {
    localStorage.setItem(KEY, "masonry");
    const { host } = await mount();

    expect(host.textContent).toBe("thumbnails");
  });

  it("writes json on setState, including functional updates", async () => {
    const { host } = await mount();

    await setMode("table");
    expect(host.textContent).toBe("table");
    expect(localStorage.getItem(KEY)).toBe(JSON.stringify("table"));

    await setMode((prev) => (prev === "table" ? "list" : prev));
    expect(host.textContent).toBe("list");
    expect(localStorage.getItem(KEY)).toBe(JSON.stringify("list"));
  });

  it("keeps the mounted value when another tab writes the same key", async () => {
    const { host } = await mount();
    localStorage.setItem(KEY, JSON.stringify("masonry"));
    window.dispatchEvent(new StorageEvent("storage", { key: KEY }));

    expect(host.textContent).toBe("thumbnails");
  });
});
