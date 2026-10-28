import { useEffect } from "react";

type PageTitleProps = {
  title: string;
};

/**
 * Sets the document title to `title` plus the app name while mounted, and
 * restores the previous one on unmount. Renders nothing.
 */
export const PageTitle = ({ title }: PageTitleProps) => {
  useEffect(() => {
    const previous = document.title;
    document.title = `${title}${TITLE_SUFFIX}`;
    return () => {
      document.title = previous;
    };
  }, [title]);

  return null;
};

const TITLE_SUFFIX = " | NASpresso";
