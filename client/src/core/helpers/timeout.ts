/** Cancel the timeout stored in `timer` and drop the id. */
export const clearRefTimeout = (timer: { current: number | null }): void => {
  if (timer.current == null) return;
  window.clearTimeout(timer.current);
  timer.current = null;
};
