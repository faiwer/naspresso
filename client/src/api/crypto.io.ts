import { z } from "zod";

/** `POST /crypto/unlock`. `expiredAt` is Unix milliseconds. */
export const UnlockIo = z.object({
  expiredAt: z.number().nullable(),
  unlocked: z.boolean(),
  /** `null` when the directory opened. */
  error: z.string().nullable(),
});

export type Unlock = z.infer<typeof UnlockIo>;
