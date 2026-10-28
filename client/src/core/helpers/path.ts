/** `photo.JPG` → `jpg`. Dotfiles like `.env` and extensionless names have none. */
export const getExtension = (name: string): string => {
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(dot + 1).toLowerCase() : "";
};

/** `["a", "b"]` → `/a/b`; `[]` → `/`. */
export const joinPath = (segments: string[]): string =>
  "/" + segments.join("/");
