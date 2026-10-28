import type { ReactNode } from "react";
import styles from "./Frame.module.css";

/** Full-height centered page frame; its content scrolls inside, the page never does. */
export const Frame = ({ children }: FrameProps) => (
  <div className={styles.root}>
    <main className={styles.frame}>{children}</main>
  </div>
);

type FrameProps = {
  children: ReactNode;
};
