import { z } from "zod";

/** `GET /health`. */
export const HealthIo = z.object({
  status: z.string(),
});

export type Health = z.infer<typeof HealthIo>;
