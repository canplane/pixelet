// @pxl/client/script/proxy.ts

/**
 * What a dev server and the page's preview (script/web.ts `serve`) answer
 * besides the bundle: the routes the page asks of its own origin, forwarded
 * to wherever the tiles come from. A device's console is answered by
 * @pxl/build/console.ts instead. Expo's transform loads this file for
 * metro.config.ts, and Node loads it too, so it must not read `import.meta`.
 */

import * as http from "node:http";
import * as https from "node:https";
import process from "node:process";

import {
  API_PREFIX,
  ASSET_PREFIX,
  GEO_PREFIX,
  SITE_URL,
  TILE_PREFIX,
} from "@pxl/client/service/origin/route.spec.ts";

/**
 * Where a dev server forwards a route the page asks of its own origin. The
 * routes split as the Worker splits them, and where they go depends on the
 * source the tiles come from, PXL_TILE_SOURCE, which the Makefile's `dev`,
 * `dev-oven` and `dev-shelf` set. Under `delivery`, every route goes to the
 * published deployment (PXL_REMOTE_ORIGIN, or else the site). Under `oven` and
 * `shelf`, the routes go to the local servers: the tiles and the assets to
 * the tile server, the API to the API server, and the rest to the search
 * server.
 */
export function route_target(url: string): string | null {
  const local = {
    geo: process.env.PXL_GEO_BASE_URL || "http://127.0.0.1:3000",
    tile: process.env.PXL_TILE_BASE_URL || "http://127.0.0.1:3001",
    api: process.env.PXL_API_BASE_URL || "http://127.0.0.1:3002",
  };
  const delivery = (process.env.PXL_TILE_SOURCE || "delivery") === "delivery";
  const routes: [string, string][] = [
    [TILE_PREFIX, local.tile],
    [ASSET_PREFIX, local.tile],
    [GEO_PREFIX, local.geo],
    [API_PREFIX, local.api],
  ];
  const hit = routes.find(([prefix]) => url.startsWith(prefix));
  if (!hit) return null;
  return delivery ? process.env.PXL_REMOTE_ORIGIN || SITE_URL : hit[1];
}

/** Forwards the request whole and unrewritten. The answer is marked
 * `no-store`, so that the browser never keeps it. The objects behind it are
 * served as `immutable` for a year, and a restart of this server can cut a
 * body in flight, which the browser would then keep as if it were whole. */
export function forward(
  target: string,
  req: http.IncomingMessage,
  res: http.ServerResponse,
): void {
  const url = new URL(req.url ?? "/", target);
  const send = url.protocol === "https:" ? https.request : http.request;
  const up = send(
    url,
    { method: req.method, headers: { ...req.headers, host: url.host } },
    (answer) => {
      res.writeHead(answer.statusCode ?? 502, {
        ...answer.headers,
        "cache-control": "no-store",
      });
      answer.pipe(res);
    },
  );
  up.on("error", (err) => {
    res.statusCode = 502;
    res.end(String(err));
  });
  req.pipe(up);
}

export type Middleware = (
  req: http.IncomingMessage,
  res: http.ServerResponse,
  next: () => void,
) => void;
