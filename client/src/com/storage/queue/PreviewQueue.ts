import { nullthrows } from "../../../core/helpers/nullthrows";
import {
  ABSENT,
  type Job,
  type PreviewSnapshot,
  type PreviewTarget,
  type Zone,
} from "./types";
import { zoneOf, zoneRank } from "./zone";

export class PreviewQueue {
  /** How many uncached previews may download at once. */
  static PREVIEW_SLOTS = 6;

  /** Grows the scrollport by half its height above and below. */
  static ROOT_MARGIN = "50% 0px";

  #observer: IntersectionObserver | null = null;
  #disposed = false;
  #drainScheduled = false;
  #jobs = new Map<string, Job>();
  #byElement = new Map<Element, Job>();
  #listeners = new Map<string, () => void>();
  #root: HTMLElement;

  constructor(root: HTMLElement) {
    this.#root = root;
  }

  subscribe(id: string, onStoreChange: () => void): () => void {
    this.#listeners.set(id, onStoreChange);
    return () => void this.#listeners.delete(id);
  }

  snapshot(id: string): PreviewSnapshot {
    return (
      this.#jobs.get(id)?.snapshot ??
      // .snapshot() is called before .attach() in a React component
      ABSENT
    );
  }

  attach(element: HTMLElement, target: PreviewTarget): () => void {
    if (this.#jobs.has(target.id)) {
      throw new Error(`Preview job ${target.id} is already attached`);
    }

    const job: Job = {
      ...target,
      zone: "out",
      active: false,
      status: "pending",
      attempt: 0,
      element,
      snapshot: ABSENT, // IO callback will actualize this
    };
    this.#jobs.set(target.id, job);
    this.#byElement.set(element, job);
    // .observe() will actualize .zone
    this.#ensureObserver().observe(element);

    return () => this.#detach(target.id);
  }

  settle(id: string, attempt: number): void {
    const job = this.#jobs.get(id);

    if (job == null || job.status === "resolved" || job.attempt !== attempt) {
      // A dead <img/> still fires `load` and `error` events. The `attempt`
      // check allows us to ignore them in favor to the new mount.
      return;
    }

    if (!job.active) {
      throw new Error(`Preview job ${id} settled while inactive`);
    }

    job.status = "resolved";
    job.active = false;
    this.#publish(job);
    // This one is ready, let the queue drain for more.
    this.#drain();
  }

  disconnect(): void {
    this.#disposed = true;
    this.#observer?.disconnect();
    this.#observer = null;
    this.#jobs.clear();
    this.#byElement.clear();
  }

  #notify(id: string): void {
    nullthrows(this.#listeners.get(id))();
  }

  // A job was updated, notify the listeners.
  #publish(job: Job): void {
    // Whether the <img src="%s"/> should be mounted.
    const enabled = // The download is complete or failed
      job.status === "resolved" ||
      // Or the download is in progress
      job.active;
    if (
      job.snapshot.enabled === enabled &&
      job.snapshot.attempt === job.attempt
    ) {
      return;
    }

    job.snapshot = { enabled, attempt: job.attempt };
    this.#notify(job.id);
  }

  #drain(): void {
    let slots = 0;
    const waiting: Job[] = [];

    for (const job of this.#jobs.values()) {
      if (job.status === "resolved") {
        continue;
      }

      if (job.active) {
        slots += 1;
      } else if (job.zone !== "out") {
        waiting.push(job);
      }
    }

    waiting.sort(
      (a, b) => zoneRank[a.zone] - zoneRank[b.zone] || a.index - b.index,
    );

    for (const job of waiting) {
      if (slots >= PreviewQueue.PREVIEW_SLOTS) {
        break;
      }

      // Let this image start downloading.
      job.active = true;
      slots += 1;
      this.#publish(job);
    }
  }

  // Unmount detaches tiles before the queue is dropped. Refill on a microtask
  // so a teardown does not start downloads for tiles that are going away.
  #scheduleDrain(): void {
    if (this.#drainScheduled) {
      return;
    }

    this.#drainScheduled = true;
    queueMicrotask(() => {
      this.#drainScheduled = false;
      if (!this.#disposed) {
        this.#drain();
      }
    });
  }

  #setZone(job: Job, zone: Zone): void {
    if (job.zone === zone) {
      return;
    }
    job.zone = zone;

    if (job.status === "resolved") {
      return;
    }

    // Image was scrolled out of view, stop downloading. Why? The preview
    // creation might have not yet started, because the server also has a queue.
    // Removing the image from the server queue allows to prioritize a really
    // visible image.
    if (zone === "out" && job.active) {
      job.active = false;
      job.attempt += 1;
    }

    this.#publish(job);
  }

  #onIntersect(entries: IntersectionObserverEntry[]): void {
    const port = this.#root.getBoundingClientRect();
    for (const entry of entries) {
      const job = nullthrows(this.#byElement.get(entry.target));
      this.#setZone(job, zoneOf(entry, port));
    }
    this.#drain();
  }

  // Lazy to avoid creating an observer for empty liss or list that have only
  // cached previews.
  #ensureObserver(): IntersectionObserver {
    return (this.#observer ??= new IntersectionObserver(
      (entries) => this.#onIntersect(entries),
      { root: this.#root, rootMargin: PreviewQueue.ROOT_MARGIN },
    ));
  }

  #detach(id: string): void {
    const job = this.#jobs.get(id);
    if (job == null) {
      return; // `disconnect` runs in a layout cleanup, before this effect cleanup.
    }

    nullthrows(this.#observer).unobserve(job.element);
    this.#byElement.delete(job.element);

    const wasActive = job.active;
    this.#jobs.delete(id);
    if (wasActive) {
      this.#scheduleDrain();
    }
  }
}
