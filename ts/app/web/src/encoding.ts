// @pxl/web/encoding.ts

/**
 * Whether this browser reads the encoding every baked object is stored in.
 * The zone refuses a browser that does not offer zstd before the Worker runs
 * (a WAF rule over the tile route), so a refused declaration is the browser's
 * answer; asked once, beside WebGPU, before anything starts. The device app
 * asks for zstd and unpacks it itself, so it has no such check.
 */

import { TILE_SOURCE_ROUTE } from "@pxl/client/driver/namespace/route.ts";

const REFUSED = 403;

export const ENCODING = "web.encoding";

/** `ENCODING` where the edge refused this browser; none where it answered,
 * or could not be reached, which the source's own ask then meets. */
export async function missing(): Promise<typeof ENCODING | null> {
  try {
    const res = await fetch(TILE_SOURCE_ROUTE, { method: "HEAD" });
    if (res.status !== REFUSED) return null;
  } catch {
    return null;
  }
  console.error("[web] the edge refused this browser: it does not offer zstd");
  return ENCODING;
}
