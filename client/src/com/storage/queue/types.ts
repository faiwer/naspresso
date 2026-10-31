export type PreviewSnapshot = {
  /** If true — <img src="%s" should be set up to download. */
  enabled: boolean;
  /** Bumped when a download is aborted, so a late `load` or `error` is ignored. */
  attempt: number;
};

export const ABSENT: PreviewSnapshot = { enabled: false, attempt: 0 };

export type Zone =
  // Outside of IntersectionObserver area. No download yet.
  | "out"
  // Above it. The least priority.
  | "above"
  // Right in the viewport. The highest priority.
  | "visible"
  // Below it. The second highest priority.
  | "below";

export type PreviewTarget = {
  id: string;
  /** Position in the feed. Waiting jobs start in this order within a zone. */
  index: number;
};

export type Job = PreviewTarget & {
  /** The zone depends on the element position in the viewport. It affects the
   * queue priority. */
  zone: Zone;
  /** Is being downloaded. */
  active: boolean;
  /** "resolved" === has the download completed successfully or failed. */
  status: "pending" | "resolved";
  /** Since detached <img/> tags still fire `load` and `error` events we need
   * this field to separate them from the new mount. */
  attempt: number;
  element: HTMLElement;
  snapshot: PreviewSnapshot;
};
