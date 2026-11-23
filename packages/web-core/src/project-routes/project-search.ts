import { zodValidator } from '@tanstack/zod-adapter';
import { z } from 'zod';

export const projectSearchSchema = z.object({
  // Preserve malformed IDs so the board can explain why the link cannot apply.
  savedFilter: z.string().optional(),
  session_id: z.string().optional(),
  agent_run_id: z.string().uuid().optional(),
});

export type ProjectSearch = z.infer<typeof projectSearchSchema>;

export const projectSearchValidator = zodValidator(projectSearchSchema);
