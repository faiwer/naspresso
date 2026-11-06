import { z } from "zod";

export const genClientErrorIo = <T extends string>() =>
  z.object({
    code: z.string() as unknown as z.ZodLiteral<T | "__UNKNOWN__">,
    message: z.string(),
  });
