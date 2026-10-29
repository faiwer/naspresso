import { Alert, Button } from "antd";
import type { CSSProperties } from "react";

import styles from "./ErrorToast.module.css";

type ErrorToastProps = {
  error: Error;
  /** Shows a Retry button when given. */
  onRetry?: () => void;
};

/** A failure panel: an illustration, "Something went wrong", the message, and an optional Retry. */
export const ErrorToast = ({ error, onRetry }: ErrorToastProps) => (
  <Alert
    type="error"
    showIcon
    icon={<img className={styles.image} src="/error.svg" alt="" />}
    title={<h3 className={styles.title}>Something went wrong</h3>}
    description={
      <>
        <p className={styles.message}>{error.message}</p>
        {onRetry && (
          <Button size="small" onClick={onRetry}>
            Retry
          </Button>
        )}
      </>
    }
    styles={ALERT_STYLES}
  />
);

// Inline, so they win over antd's own alert layout regardless of stylesheet order.
const ALERT_STYLES: Partial<Record<"root" | "icon", CSSProperties>> = {
  root: {
    alignItems: "center",
    "--ant-alert-with-description-padding": "0.5rem 2rem 0.5rem 0rem",
    width: "max-content",
  },
  icon: { marginInlineEnd: "2rem" },
};
