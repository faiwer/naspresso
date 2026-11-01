import type { FrameSide } from "../frameEnabled";
import { type SideDownloaded } from "./gesture";

export type Motion = {
  /** Track offset in pixels. Positive follows a finger moving toward the previous image. */
  translateX: number;
  /** The release animation is running, so new touches are ignored. */
  settling: boolean;
  /** Neighbor sliding into the center, so it loads immediately. */
  promoted: FrameSide | null;
  /** Index step applied when the gesture lands: `-1` previous, `1` next, `0` a rollback. */
  delta: -1 | 0 | 1;
  /** Which of the three slots have finished loading. */
  downloaded: SideDownloaded;
};

export const initialMotion: Motion = {
  translateX: 0,
  settling: false,
  promoted: null,
  delta: 0,
  downloaded: { left: false, center: false, right: false },
};
