// @pxl/api/app/config.ts

import type { i32 } from "@xpute/core/kit/word.ts";

import * as env from "@/lib/env/mod.ts";

export type AppConfig = {
  max_body_sz: i32;
  port: i32;
  host: string;
  socket_path: string;
};

export function make_cfg(): AppConfig {
  const max_body_sz_mb: i32 = env.must_int("MAX_BODY_SZ_MB", { fallback: 1, min: 1 });
  const max_body_sz: i32 = (max_body_sz_mb * (1 << 20)) as i32;

  const port: i32 = env.must_int("API_PORT", { fallback: 3000, min: 1, max: 65535 });

  // keep literals centralized (override later if you want)
  const host = env.get("API_HOST") || "0.0.0.0";
  const socket_path = env.get("SOCKET_PATH") || "/io";

  return { max_body_sz, port, host, socket_path };
}
