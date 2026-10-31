import { createContext, useContext } from "react";

import { nullthrows } from "../../../core/helpers/nullthrows";
import type { PreviewQueue } from "./PreviewQueue";

export const PreviewQueueContext = createContext<PreviewQueue | null>(null);

/** The listing's queue. Throws outside {@link PreviewQueueProvider}. */
export const usePreviewQueueContextX = (): PreviewQueue =>
  nullthrows(
    useContext(PreviewQueueContext),
    "usePreviewQueueContextX must be used inside a preview queue",
  );
