/** The document's root `font-size` in px, or 16 if it can't be read. */
export const rootFontSize = (): number => {
  const size = parseFloat(getComputedStyle(document.documentElement).fontSize);
  return Number.isFinite(size) && size > 0 ? size : 16;
};
