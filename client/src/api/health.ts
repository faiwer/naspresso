import { apiGet, type ApiRequestOptions } from "./core";
import { HealthIo, type Health } from "./health.io";

/** `GET /health`: whether the server is up. */
export const getHealth = (options: ApiRequestOptions = {}): Promise<Health> =>
  apiGet("/health", HealthIo, options);
