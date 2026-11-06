import type { CSSProperties } from "react";

// Inline, so they win over antd's own alert layout regardless of stylesheet order.
export const ALERT_STYLES: Partial<Record<"root" | "icon", CSSProperties>> = {
  root: { alignItems: "flex-start" },
  icon: { marginTop: "0.15rem" },
};
