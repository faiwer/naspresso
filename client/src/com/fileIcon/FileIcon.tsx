import frameUrl from "./frame.svg";
import { getMimeIcon } from "./getMimeIcon";

import styles from "./FileIcon.module.css";

type FileIconProps = {
  mime: string;
};

/** A file page with its type glyph centered on the sheet. */
export const FileIcon = ({ mime }: FileIconProps) => (
  <div className={styles.root}>
    <img className={styles.frame} src={frameUrl} alt="" />
    {getMimeIcon(mime, styles.glyph)}
  </div>
);
