/** Structural equality of plain data: primitives, arrays and plain objects. */
export const isDeepEqual = (a: unknown, b: unknown): boolean => {
  if (Object.is(a, b)) {
    return true;
  }

  if (!a || !b || typeof a !== "object" || typeof b !== "object") {
    return false;
  }

  if (Array.isArray(a) || Array.isArray(b)) {
    return (
      Array.isArray(a) &&
      Array.isArray(b) &&
      a.length === b.length &&
      a.every((item, index) => isDeepEqual(item, b[index]))
    );
  }

  if (Object.getPrototypeOf(a) !== Object.getPrototypeOf(b)) {
    return false;
  }

  const aRecord = a as Record<string, unknown>;
  const bRecord = b as Record<string, unknown>;
  const keys = Object.keys(aRecord);
  return (
    keys.length === Object.keys(bRecord).length &&
    keys.every(
      (key) =>
        Object.hasOwn(bRecord, key) && isDeepEqual(aRecord[key], bRecord[key]),
    )
  );
};

/** One level of {@link isDeepEqual}: nested values are compared with `Object.is`. */
export const isShallowEqual = (a: unknown, b: unknown): boolean => {
  if (Object.is(a, b)) {
    return true;
  }

  if (!a || !b || typeof a !== "object" || typeof b !== "object") {
    return false;
  }

  if (Array.isArray(a) || Array.isArray(b)) {
    return (
      Array.isArray(a) &&
      Array.isArray(b) &&
      a.length === b.length &&
      a.every((item, index) => Object.is(item, b[index]))
    );
  }

  if (Object.getPrototypeOf(a) !== Object.getPrototypeOf(b)) {
    return false;
  }

  const aRecord = a as Record<string, unknown>;
  const bRecord = b as Record<string, unknown>;
  const keys = Object.keys(aRecord);
  return (
    keys.length === Object.keys(bRecord).length &&
    keys.every(
      (key) =>
        Object.hasOwn(bRecord, key) && Object.is(aRecord[key], bRecord[key]),
    )
  );
};

export const updateIfChanged = <T>(prev: T, next: T): T =>
  isShallowEqual(prev, next) ? prev : next;
