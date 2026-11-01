import clsx from "clsx";

import frameUrl from "./frame.svg?react";
import frameExtUrl from "./frameExt.svg?react";
import { getMimeIcon } from "./getMimeIcon";

import styles from "./FileIcon.module.css";

type FileIconProps = {
  mime: string;
  /** Extension without the dot. Empty when the name has none. */
  ext: string;
};

/** A file page with its type glyph centered on the sheet. */
export const FileIcon = ({ mime, ext }: FileIconProps) => {
  const labeled = ext.length > 0 && ext.length <= EXT_LABEL_MAX;
  const Box = labeled ? frameExtUrl : frameUrl;
  return (
    <div className={clsx(styles.root, labeled && styles.labeled)}>
      <Box className={styles.frame} />
      {getMimeIcon(mime, styles.glyph)}
      {labeled && <span className={styles.ext}>{ext}</span>}
    </div>
  );
};

/** Extensions longer than this use the plain page, with no label. */
const EXT_LABEL_MAX = 3;
