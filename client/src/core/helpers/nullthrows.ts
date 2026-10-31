/**
 * Narrow `value` to a non-nullish type or throw. Use it instead of
 * the `!` non-null assertion operator: `!` lies to the type system
 * with no runtime check, so when reality disagrees the bug surfaces
 * far from the cause as a confusing `TypeError: cannot read property
 * of undefined`. `nullthrows` fails loud, at the assertion site, with
 * a stack trace.
 *
 * Reach for it when you have a structural reason to believe the value
 * is present (an effect that only runs after a ref is attached, a
 * route param the router guarantees, a key just `set` in a `Map`).
 * For values that legitimately may be missing, use a normal
 * `if (!x) return ...` with code that handles the absence.
 */
export function nullthrows<T>(
  value: T | null | undefined,
  message?: Error | string,
): T {
  if (value !== null && value !== undefined) {
    return value;
  }

  if (message instanceof Error) {
    throw message;
  }

  throw new Error(message ?? `Expected non-nullish value, got ${value}`);
}
