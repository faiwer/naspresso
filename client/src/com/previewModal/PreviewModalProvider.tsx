import { useMemo, useState, useStableCallback, type ReactNode } from "react";

import type { StorageFile } from "../../api/storage.io";
import { nullthrows } from "../../core/helpers/nullthrows";
import { PreviewModalContext, type PreviewSession } from "./context";
import { PreviewModal } from "./PreviewModal";

type PreviewModalProviderProps = {
  children: ReactNode;
};

/** App-wide preview modal. Mount once, around the whole tree. */
export const PreviewModalProvider = ({
  children,
}: PreviewModalProviderProps) => {
  const [session, setSession] = useState<PreviewSession | null>(null);
  const openPreviewModal = useStableCallback(
    (files: StorageFile[], pos: number, segments: string[]) => {
      nullthrows(files[pos], "File not found");
      setSession({ files, pos, segments });
    },
  );
  const close = useStableCallback(() => setSession(null));
  const api = useMemo(() => ({ openPreviewModal }), [openPreviewModal]);

  return (
    <PreviewModalContext.Provider value={api}>
      {children}
      {session && <PreviewModal session={session} onClose={close} />}
    </PreviewModalContext.Provider>
  );
};
