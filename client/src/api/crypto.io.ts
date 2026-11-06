import { z } from "zod";

import { genClientErrorIo } from "./core.io";

/** `POST /crypto/unlock`. `expiredAt` is Unix milliseconds. */
export const UnlockIo = z.object({
  expiredAt: z.number().nullable(),
  unlocked: z.boolean(),
  /** `null` when the directory opened. */
  error: genClientErrorIo<"NoEncryptedFile" | "WrongPassword">().nullable(),
});

export type Unlock = z.infer<typeof UnlockIo>;
