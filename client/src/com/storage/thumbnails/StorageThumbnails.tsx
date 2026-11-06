import { getOriginalEntryName } from "../../../api/storage";
import { range } from "../../../core/helpers/array";
import {
  PreviewQueueProvider,
  usePreviewQueue,
} from "../queue/usePreviewQueue";
import type { StorageViewProps } from "../StorageList";
import { ThumbnailSkeleton } from "./skeletons";
import { Thumbnail } from "./Thumbnail";

import styles from "./StorageThumbnails.module.css";

export const StorageThumbnails = ({
  segments,
  entries,
  skeletons,
  scrollContainer,
  openImage,
  encryption,
}: StorageViewProps) => {
  const queue = usePreviewQueue(scrollContainer);
  return (
    <PreviewQueueProvider queue={queue}>
      <div className={styles.grid}>
        {entries.map((entry, index) => (
          <Thumbnail
            key={getOriginalEntryName(entry)}
            entry={entry}
            index={index}
            segments={segments}
            openImage={openImage}
            encryption={encryption}
          />
        ))}
        {range(skeletons).map((index) => (
          <ThumbnailSkeleton key={`skeleton-${index}`} />
        ))}
      </div>
    </PreviewQueueProvider>
  );
};
