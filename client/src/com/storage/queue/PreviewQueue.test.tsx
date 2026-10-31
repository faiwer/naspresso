import { createRoot, useRef } from "react";
import { act } from "react/testing";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { getPreviewSrcSet } from "../../../api/media";
import { range } from "../../../core/helpers/array";
import { nullthrows } from "../../../core/helpers/nullthrows";
import { Image } from "../../image/Image";
import { PreviewQueue } from "./PreviewQueue";
import { usePreviewJob } from "./usePreviewJob";
import { PreviewQueueProvider, usePreviewQueue } from "./usePreviewQueue";

/**
 * Scrollport the queue compares tiles against. Bands below are placed
 * relative to this box: top 0, bottom 100.
 */
const PORT = new DOMRect(0, 0, 200, 100);

type Band = "out" | "above" | "visible" | "below";

const BAND_RECT: Record<Band, DOMRect> = {
  visible: new DOMRect(0, 40, 20, 20),
  below: new DOMRect(0, 120, 20, 20),
  above: new DOMRect(0, -40, 20, 20),
  out: new DOMRect(0, 400, 20, 20),
};

/**
 * Records every `observe` and fires only when the test says where a tile
 * sits. Nothing is delivered from `observe` itself.
 */
class FakeIntersectionObserver {
  static current: FakeIntersectionObserver | null = null;

  readonly root: Element | Document | null;
  readonly rootMargin: string;
  readonly targets = new Set<Element>();
  #callback: IntersectionObserverCallback;

  constructor(
    callback: IntersectionObserverCallback,
    options?: IntersectionObserverInit,
  ) {
    this.#callback = callback;
    this.root = options?.root ?? null;
    this.rootMargin = options?.rootMargin ?? "";
    FakeIntersectionObserver.current = this;
  }

  observe(target: Element): void {
    this.targets.add(target);
  }

  unobserve(target: Element): void {
    this.targets.delete(target);
  }

  disconnect(): void {
    this.targets.clear();
  }

  place(placements: { target: Element; band: Band }[]): void {
    const entries = placements.map(({ target, band }) => {
      const rect = BAND_RECT[band];
      const entry: IntersectionObserverEntry = {
        target,
        isIntersecting: band !== "out",
        boundingClientRect: rect,
        intersectionRatio: band === "out" ? 0 : 1,
        intersectionRect: band === "out" ? new DOMRect() : rect,
        rootBounds: PORT,
        time: 0,
      };
      return entry;
    });
    this.#callback(entries, this as unknown as IntersectionObserver);
  }
}

type TileSpec = {
  filePath: string;
  index: number;
  hasPreview?: boolean;
};

const Tile = ({ tile }: { tile: TileSpec }) => {
  const ref = useRef<HTMLDivElement>(null);
  const job = usePreviewJob(
    ref,
    tile.filePath,
    tile.index,
    tile.hasPreview ?? false,
  );
  return (
    <div ref={ref} data-file={tile.filePath}>
      <Image
        width={80}
        height={80}
        srcSet={getPreviewSrcSet(tile.filePath)}
        enabled={job.enabled}
        onLoad={job.onLoad}
        onError={job.onError}
      />
    </div>
  );
};

const Listing = ({
  scroll,
  tiles,
}: {
  scroll: HTMLElement;
  tiles: TileSpec[];
}) => {
  const queue = usePreviewQueue(scroll);
  return (
    <PreviewQueueProvider queue={queue}>
      <div>
        {tiles.map((tile) => (
          <Tile key={tile.filePath} tile={tile} />
        ))}
      </div>
    </PreviewQueueProvider>
  );
};

/**
 * `act` finishes the render scheduled by its callback. `useEffect` runs
 * on a later timer, and a layout effect may schedule one more render.
 */
const settle = async (): Promise<void> => {
  for (let i = 0; i < 8; i += 1) {
    await act(async () => {
      await new Promise<void>((resolve) => {
        setTimeout(resolve, 0);
      });
    });
  }
};

type View = {
  scroll: HTMLElement;
  place: (bands: Record<string, Band>) => Promise<void>;
  image: (filePath: string) => HTMLImageElement | null;
  src: (filePath: string) => string | null;
  showsError: (filePath: string) => boolean;
  observed: () => string[];
  observer: () => FakeIntersectionObserver;
};

let cleanup: (() => void) | null = null;

const mount = async (tiles: TileSpec[]): Promise<View> => {
  cleanup?.();
  const scroll = document.createElement("div");
  scroll.getBoundingClientRect = () => PORT;
  const host = document.createElement("div");
  document.body.appendChild(host);
  const root = createRoot(host, { testMode: true });
  root.render(<Listing scroll={scroll} tiles={tiles} />);
  cleanup = () => {
    root.unmount();
    host.remove();
  };

  const tile = (filePath: string): Element =>
    nullthrows(
      host.querySelector(`[data-file="${filePath}"]`),
      `Missing tile ${filePath}`,
    );

  const image = (filePath: string): HTMLImageElement | null => {
    for (const img of tile(filePath).querySelectorAll("img")) {
      if (img.getAttribute("src") !== "/error.svg") {
        return img;
      }
    }
    return null;
  };

  const view: View = {
    scroll,
    image,
    src: (filePath) => image(filePath)?.getAttribute("src") ?? null,
    showsError: (filePath) =>
      tile(filePath).querySelector("img[src='/error.svg']") != null,
    observed: () =>
      [...nullthrows(FakeIntersectionObserver.current).targets].map((element) =>
        nullthrows(element.getAttribute("data-file")),
      ),
    observer: () => nullthrows(FakeIntersectionObserver.current),
    place: async (bands) => {
      nullthrows(FakeIntersectionObserver.current).place(
        Object.entries(bands).map(([filePath, band]) => ({
          target: tile(filePath),
          band,
        })),
      );
      await settle();
    },
  };

  await settle();
  return view;
};

beforeEach(() => {
  FakeIntersectionObserver.current = null;
  vi.stubGlobal("IntersectionObserver", FakeIntersectionObserver);
  HTMLImageElement.prototype.decode = () => Promise.resolve();
});

afterEach(async () => {
  cleanup?.();
  cleanup = null;
  await settle();
  vi.unstubAllGlobals();
});

const visible = (indexes: number[]): TileSpec[] =>
  indexes.map((index) => ({ filePath: `/v${index}.jpg`, index }));

describe("preview queue", () => {
  it("shows a cached preview immediately and does not observe it", async () => {
    const view = await mount([
      { filePath: "/cached.jpg", index: 0, hasPreview: true },
      { filePath: "/fresh.jpg", index: 1 },
    ]);

    expect(view.src("/cached.jpg")).toContain("/cached.jpg");
    expect(view.src("/fresh.jpg")).toBeNull();
    expect(view.observed()).toEqual(["/fresh.jpg"]);
    expect(view.observer().root).toBe(view.scroll);
    expect(view.observer().rootMargin).toBe(PreviewQueue.ROOT_MARGIN);
  });

  it("starts visible tiles in feed order, up to the slot limit", async () => {
    const queued = range(PreviewQueue.PREVIEW_SLOTS + 1).map((index) => ({
      filePath: `/queue-${index}.jpg`,
      index,
    }));
    const view = await mount([
      { filePath: "/cached.jpg", index: 100, hasPreview: true },
      ...queued.toReversed(),
    ]);

    await view.place(
      Object.fromEntries(queued.map((tile) => [tile.filePath, "visible"])),
    );

    for (const tile of queued.slice(0, PreviewQueue.PREVIEW_SLOTS)) {
      expect(view.src(tile.filePath)).toContain(tile.filePath);
    }
    expect(view.src(`/queue-${PreviewQueue.PREVIEW_SLOTS}.jpg`)).toBeNull();
    expect(view.src("/cached.jpg")).toContain("/cached.jpg");
    expect(view.observed()).not.toContain("/cached.jpg");
  });

  it("starts visible, then below, then above, and skips tiles outside the band", async () => {
    const view = await mount([
      { filePath: "/above-21.jpg", index: 21 },
      { filePath: "/above-20.jpg", index: 20 },
      { filePath: "/below-10.jpg", index: 10 },
      { filePath: "/visible-3.jpg", index: 3 },
      { filePath: "/visible-1.jpg", index: 1 },
      { filePath: "/visible-0.jpg", index: 0 },
      { filePath: "/visible-2.jpg", index: 2 },
      { filePath: "/out.jpg", index: 30 },
    ]);

    await view.place({
      "/above-21.jpg": "above",
      "/above-20.jpg": "above",
      "/below-10.jpg": "below",
      "/visible-3.jpg": "visible",
      "/visible-1.jpg": "visible",
      "/visible-0.jpg": "visible",
      "/visible-2.jpg": "visible",
      "/out.jpg": "out",
    });

    for (const filePath of [
      "/visible-0.jpg",
      "/visible-1.jpg",
      "/visible-2.jpg",
      "/visible-3.jpg",
      "/below-10.jpg",
      "/above-20.jpg",
    ]) {
      expect(view.src(filePath)).toContain(filePath);
    }
    expect(view.src("/above-21.jpg")).toBeNull();
    expect(view.src("/out.jpg")).toBeNull();
  });

  it("aborts a download that leaves the band and gives the slot to the next waiter", async () => {
    const view = await mount([
      ...visible(range(PreviewQueue.PREVIEW_SLOTS)),
      { filePath: "/below.jpg", index: 100 },
    ]);
    const bands: Record<string, Band> = {
      ...Object.fromEntries(
        range(PreviewQueue.PREVIEW_SLOTS).map((index) => [
          `/v${index}.jpg`,
          "visible",
        ]),
      ),
      "/below.jpg": "below",
    };
    await view.place(bands);

    const v0 = view.image("/v0.jpg");
    expect(view.src("/v0.jpg")).toContain("/v0.jpg");
    expect(view.src("/below.jpg")).toBeNull();

    await view.place({ "/v0.jpg": "out" });
    expect(view.src("/v0.jpg")).toBeNull();
    expect(view.src("/below.jpg")).toContain("/below.jpg");
    expect(v0?.getAttribute("src")).toBeFalsy();

    await view.place({ "/v0.jpg": "visible" });
    expect(view.src("/v0.jpg")).toBeNull();

    nullthrows(view.image("/v1.jpg")).dispatchEvent(new Event("load"));
    await settle();
    expect(view.src("/v1.jpg")).toContain("/v1.jpg");
    expect(view.src("/v0.jpg")).toContain("/v0.jpg");
  });

  it("keeps a download that is already running when its rank gets worse", async () => {
    const view = await mount([
      ...visible(range(PreviewQueue.PREVIEW_SLOTS)),
      { filePath: "/below.jpg", index: 100 },
    ]);
    await view.place({
      ...Object.fromEntries(
        range(PreviewQueue.PREVIEW_SLOTS).map((index) => [
          `/v${index}.jpg`,
          "visible",
        ]),
      ),
      "/below.jpg": "below",
    });

    const running = nullthrows(view.image("/v0.jpg"));
    await view.place({ "/v0.jpg": "above" });

    expect(view.image("/v0.jpg")).toBe(running);
    expect(view.src("/below.jpg")).toBeNull();
  });

  it("treats an error as finished and does not request that preview again", async () => {
    const view = await mount([
      ...visible(range(PreviewQueue.PREVIEW_SLOTS)),
      { filePath: "/below.jpg", index: 100 },
    ]);
    await view.place({
      ...Object.fromEntries(
        range(PreviewQueue.PREVIEW_SLOTS).map((index) => [
          `/v${index}.jpg`,
          "visible",
        ]),
      ),
      "/below.jpg": "below",
    });

    const failed = nullthrows(view.image("/v0.jpg"));
    failed.dispatchEvent(new Event("error"));
    await settle();

    expect(view.showsError("/v0.jpg")).toBe(true);
    expect(view.image("/v0.jpg")).toBeNull();
    expect(view.src("/below.jpg")).toContain("/below.jpg");

    await view.place({ "/v0.jpg": "out" });
    await view.place({ "/v0.jpg": "visible" });
    expect(view.image("/v0.jpg")).toBeNull();
    expect(view.showsError("/v0.jpg")).toBe(true);
  });
});
