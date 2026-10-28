/** Lets the eventual sink (Sentry, a log endpoint, …) group and filter errors. */
export interface TrackErrorContext {
  /** Coarse area of the app: `"storage"`, `"home"`, … */
  feature: string;
  /** Where in that area the error came from: hook or function name. */
  origin: string;
  /** Free-form extras (path, request URL, …). No PII. */
  extra?: Record<string, unknown>;
}

/** Single sink for errors nobody handled. Only logs until there's a telemetry pipeline. */
export const trackError = (err: unknown, ctx: TrackErrorContext): void => {
  console.error(`[${ctx.feature}/${ctx.origin}]`, err, ctx.extra);
};
