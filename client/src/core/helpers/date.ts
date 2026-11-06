import { useEffect, useState } from "react";

/** Medium date and short time in the user's locale, e.g. `Oct 28, 2026, 7:00 PM`. */
export const formatDateTime = (timestamp: number): string =>
  dateTimeFormat.format(timestamp);

/** `remainingMs` as whole minutes, or seconds once under a minute. */
export const formatLockRemaining = (remainingMs: number): string => {
  const seconds = Math.max(0, Math.ceil(remainingMs / 1000));
  if (seconds >= 60) {
    return `${Math.floor(seconds / 60)} min`;
  }
  return `${seconds} sec`;
};

/** Milliseconds until `deadline`, refreshed once a second. */
export const useCountdown = (deadline: number): number => {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const id = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(id);
  }, []);
  return deadline - now;
};

const dateTimeFormat = new Intl.DateTimeFormat(undefined, {
  dateStyle: "medium",
  timeStyle: "short",
});
