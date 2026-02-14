import { z } from "zod";

export const toolRequestSchema = z.object({
  requestId: z.string().min(1),
  toolId: z.string().min(1),
  payload: z.unknown(),
  timeoutMs: z.number().int().positive().optional()
});

export const toolErrorSchema = z.object({
  code: z.string(),
  message: z.string(),
  details: z.unknown().optional()
});

export const toolResponseSchema = z.object({
  requestId: z.string().min(1),
  ok: z.boolean(),
  data: z.unknown().optional(),
  error: toolErrorSchema.optional(),
  meta: z
    .object({
      durationMs: z.number().nonnegative(),
      warnings: z.array(z.string()).optional()
    })
    .optional()
});

export type ToolRequest = z.infer<typeof toolRequestSchema>;
export type ToolResponse = z.infer<typeof toolResponseSchema>;

export function parseToolRequest(input: unknown): ToolRequest {
  return toolRequestSchema.parse(input);
}

export function successResponse(requestId: string, data: unknown, durationMs: number): ToolResponse {
  return {
    requestId,
    ok: true,
    data,
    meta: { durationMs }
  };
}

export function failureResponse(requestId: string, code: string, message: string, durationMs: number): ToolResponse {
  return {
    requestId,
    ok: false,
    error: { code, message },
    meta: { durationMs }
  };
}
