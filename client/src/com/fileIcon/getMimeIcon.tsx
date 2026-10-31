import { AlertFilled } from "@ant-design/icons";
import type { ReactNode } from "react";

import MountainIcon from "../icons/mountain.svg?react";

/** The antd glyph for a mime. Unlisted types share one icon for now. */
export const getMimeIcon = (mime: string, className?: string): ReactNode => {
  if (mime.startsWith("image/")) {
    return <MountainIcon className={className} />;
  }
  return <AlertFilled className={className} />;
};
