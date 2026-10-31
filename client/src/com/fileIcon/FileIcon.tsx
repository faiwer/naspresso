import { AlertFilled } from "@ant-design/icons";
import type { ReactNode } from "react";

import frameUrl from "./frame.svg";

import styles from "./FileIcon.module.css";

type FileIconProps = {
  mime: string;
};

/** A file page with its type glyph centered on the sheet. */
export const FileIcon = ({ mime }: FileIconProps) => (
  <div className={styles.root}>
    <img className={styles.frame} src={frameUrl} alt="" />
    {renderGlyph(mime)}
  </div>
);

/** The antd icon for a mime. Every type shares one icon for now. */
const renderGlyph = (_mime: string): ReactNode => (
  <AlertFilled className={styles.glyph} />
);
