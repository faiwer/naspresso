import { useState, useStableCallback } from "react";

import type { StorageFile } from "../../api/storage.io";
import { nullthrows } from "../../core/helpers/nullthrows";
import { fittedSize } from "./fittedSize";
import { isFrameEnabled } from "./frameEnabled";
import { PreviewFrame } from "./PreviewFrame";

import styles from "./PreviewModal.module.css";

type PreviewStripProps = {
  files: StorageFile[];
  pos: number;
  segments: string[];
  viewport: { width: number; height: number };
};

type Downloaded = { center: boolean; right: boolean };

/** The current image and the neighbors flush with the viewport edges. */
export const PreviewStrip = ({
  files,
  pos,
  segments,
  viewport,
}: PreviewStripProps) => {
  const center = nullthrows(files[pos]);
  const left = files[pos - 1] ?? null;
  const right = files[pos + 1] ?? null;
  const [downloaded, setDownloaded] = useState<Downloaded>({
    center: false,
    right: false,
  });
  const markCenter = useStableCallback(() =>
    setDownloaded((prev) => (prev.center ? prev : { ...prev, center: true })),
  );
  const markRight = useStableCallback(() =>
    setDownloaded((prev) => (prev.right ? prev : { ...prev, right: true })),
  );

  return (
    <div className={styles.track}>
      {left && (
        <PreviewFrame
          file={left}
          segments={segments}
          side="left"
          enabled={isFrameEnabled("left", null, downloaded, right != null)}
          {...fittedBox(left, viewport)}
        />
      )}
      <PreviewFrame
        file={center}
        segments={segments}
        side="center"
        enabled
        onLoad={markCenter}
        {...fittedBox(center, viewport)}
      />
      {right && (
        <PreviewFrame
          file={right}
          segments={segments}
          side="right"
          enabled={isFrameEnabled("right", null, downloaded, true)}
          onLoad={markRight}
          {...fittedBox(right, viewport)}
        />
      )}
    </div>
  );
};

const fittedBox = (
  file: StorageFile,
  viewport: { width: number; height: number },
) => fittedSize(nullthrows(file.image), viewport);
