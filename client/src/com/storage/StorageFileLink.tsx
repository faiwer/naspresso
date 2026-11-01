import type { ReactNode } from "react";

import { isPlainClick } from "../../core/helpers/mouse";

type StorageFileLinkProps = {
  href: string;
  className?: string;
  title?: string;
  /** Plain click opens the preview; a modified click follows `href`. */
  onPreview: (() => void) | null;
  children: ReactNode;
};

/** A file's anchor. `onPreview` handles an unmodified click; any other click follows `href`. */
export const StorageFileLink = ({
  href,
  className,
  title,
  onPreview,
  children,
}: StorageFileLinkProps) => (
  <a
    className={className}
    href={href}
    title={title}
    target="_blank"
    rel="noreferrer"
    onClick={
      onPreview
        ? (event) => {
            if (isPlainClick(event)) {
              event.preventDefault();
              onPreview();
            }
          }
        : undefined
    }
  >
    {children}
  </a>
);
