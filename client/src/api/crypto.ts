import { apiPost, apiPostVoid, type ApiRequestOptions } from "./core";
import { UnlockIo, type Unlock } from "./crypto.io";

/**
 * `POST /crypto/unlock`: open an `.nped` directory and store its key in a cookie.
 * A wrong password or a directory with no `.npe` file resolves with `unlocked: false`.
 */
export const unlockNPED = (
  path: string,
  password: string,
  options: ApiRequestOptions = {},
): Promise<Unlock> =>
  apiPost("/crypto/unlock", UnlockIo, { password, path }, options);

/**
 * `POST /crypto/lock`: drop the unlock cookie for `path`.
 * A cookie that was never set still resolves.
 */
export const lockNPED = (
  path: string,
  options: ApiRequestOptions = {},
): Promise<void> => apiPostVoid("/crypto/lock", { path }, options);

export const NPE_PASSWORD_LENGTH = 64;

/** How long the NPE-unlock cookie lasts in minutes. Keep in touch with the backend. */
export const MAX_NPE_COOKIE_DURATION_M = 15;
