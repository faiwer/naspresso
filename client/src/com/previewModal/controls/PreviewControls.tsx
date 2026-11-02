import { ArrowLeftOutlined, ArrowRightOutlined } from "@ant-design/icons";
import clsx from "clsx";

import { usePreviewChrome } from "./usePreviewChrome";

import styles from "./PreviewControls.module.css";

type PreviewControlsProps = {
  viewport: HTMLDivElement;
  pos: number;
  count: number;
  onNavigate: (delta: -1 | 1) => void;
};

/** Mouse controls over the preview: arrows and the current position. */
export const PreviewControls = ({
  viewport,
  pos,
  count,
  onNavigate,
}: PreviewControlsProps) => {
  const chrome = usePreviewChrome(viewport);

  return (
    <div
      className={clsx(styles.layer, chrome.visible && styles.visible)}
      onPointerOver={chrome.onPointerOver}
      onPointerOut={chrome.onPointerOut}
    >
      {pos > 0 && (
        <div
          className={clsx(styles.arrow, styles.left)}
          onClick={() => onNavigate(-1)}
        >
          <ArrowLeftOutlined />
        </div>
      )}
      {pos < count - 1 && (
        <div
          className={clsx(styles.arrow, styles.right)}
          onClick={() => onNavigate(1)}
        >
          <ArrowRightOutlined />
        </div>
      )}
      <div className={styles.counter}>
        {pos + 1} / {count}
      </div>
    </div>
  );
};
