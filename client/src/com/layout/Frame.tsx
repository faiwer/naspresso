import type { ReactNode } from "react";

import styles from "./Frame.module.css";

type FrameProps = {
  header?: ReactNode;
  children: ReactNode;
};

/**
 * Full-height centered page frame; its content scrolls inside, the page never does. The
 * `header` row stays put above the scrolling content; use {@link Gap} to push items apart.
 */
export const Frame = ({ header, children }: FrameProps) => (
  <div className={styles.root}>
    <main className={styles.frame}>
      {header != null && <header className={styles.header}>{header}</header>}
      <div className={styles.content}>{children}</div>
    </main>
  </div>
);
