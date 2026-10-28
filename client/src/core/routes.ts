export const routes = {
  home: () => "/",
  /** A directory of `STORAGE`, given as path segments. Spaces are written as `+`. */
  storage: (segments: string[] = []) =>
    STORAGE_URL +
    segments.map((segment) => "/" + encodeSegment(segment)).join(""),
};

/**
 * Reverses {@link routes.storage} on a raw pathname. `useParams` can't be used: it has
 * already decoded `%2B` into `+`, which would then be indistinguishable from an encoded space.
 */
export const parseStorageUrl = (pathname: string): string[] =>
  pathname
    .slice(STORAGE_URL.length)
    .split("/")
    .filter(Boolean)
    .map(decodeSegment);

const STORAGE_URL = "/storage";

/** Like `encodeURIComponent`, but a space becomes `+`. A literal `+` is still escaped as `%2B`. */
const encodeSegment = (segment: string): string =>
  encodeURIComponent(segment).replaceAll("%20", "+");

/** Reverses `encodeSegment`. `+` must become a space before `%2B` is decoded into `+`. */
const decodeSegment = (segment: string): string => {
  const spaced = segment.replaceAll("+", " ");
  try {
    return decodeURIComponent(spaced);
  } catch {
    // A malformed escape like `%E0` in a hand-typed URL. The server reports it as not found.
    return spaced;
  }
};
