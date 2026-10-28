/** Medium date and short time in the user's locale, e.g. `Oct 28, 2026, 7:00 PM`. */
export const formatDateTime = (timestamp: number): string =>
  dateTimeFormat.format(timestamp);

const dateTimeFormat = new Intl.DateTimeFormat(undefined, {
  dateStyle: "medium",
  timeStyle: "short",
});
