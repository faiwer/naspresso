import { AlignCenterOutlined } from "@ant-design/icons";
import type { ReactNode } from "react";

import BinaryIcon from "../icons/binary.svg?react";
import MountainIcon from "../icons/mountain.svg?react";
import ZipperIcon from "../icons/zipper.svg?react";

/** The antd glyph for a mime. Unlisted types share one icon for now. */
export const getMimeIcon = (mime: string, className?: string): ReactNode => {
  let Icon: typeof MountainIcon = BinaryIcon;

  if (mime.startsWith("image/")) {
    Icon = MountainIcon;
  }

  if (
    /^application\/(gzip|zip|zstd|vnd\.rar|x-(7z-compressed|bzip2|rar-compressed|tar|xz))$/.test(
      mime,
    )
  ) {
    Icon = ZipperIcon;
  }

  if (mime.startsWith("text/")) {
    Icon = AlignCenterOutlined;
  }

  return <Icon className={className} />;
};
