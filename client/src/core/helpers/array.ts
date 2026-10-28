/** `3` → `[0, 1, 2]`. */
export const range = (count: number): number[] =>
  Array.from({ length: count }, (_, index) => index);
