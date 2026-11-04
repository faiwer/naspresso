import { apiPost, type ApiRequestOptions } from "./core";
import { UnlockIo, type Unlock } from "./crypto.io";

/**
 * `POST /crypto/unlock`: open an `.nped` directory and store its key in a cookie.
 * A wrong password or a directory with no `.npe` file resolves with `unlocked: false`.
 */
export const createUnlock = (
  path: string,
  password: string,
  options: ApiRequestOptions = {},
): Promise<Unlock> =>
  apiPost("/crypto/unlock", UnlockIo, { password, path }, options);
