// @pxl/api/lib/admin_auth.ts

import type { Context, Next } from "@hono/hono";

/**
 * Shared-secret gate for low-traffic internal/ops routes (cache
 * invalidation, etc.) — not a real auth system, just enough to keep them
 * from being callable by anyone who finds the URL.
 */
export function admin_auth(token: string) {
  return async (c: Context, next: Next) => {
    if (c.req.header("x-admin-token") !== token) {
      return c.text("unauthorized", 401);
    }
    await next();
  };
}
