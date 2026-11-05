// @pxl/client/script/proxy.ts

/**
 * What a dev server and the page's preview (script/web.ts `serve`) answer
 * besides the bundle: the routes the page asks of its own origin, forwarded
 * where the tiles come from. A device's console is @pxl/build/console.ts's.
 * Loaded by Expo's transform for metro.config.ts as well as by Node, so it
 * reads no `import.meta`.
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
} from "@pxl/client/origin/route.spec.ts";

/**
 * Where a dev server forwards each route the page asks of its own origin,
 * split as the Worker splits them, by the source the tiles come from
 * (PXL_DEV_SOURCE, the Makefile's `dev`, `dev-oven`, `dev-shelf`):
 * `delivery`, the published deployment (PXL_REMOTE_ORIGIN, else the site);
 * `oven` and `shelf`, the local servers, the tiles' and assets' to the tile
 * server, the API to the API server, the rest to the search server.
 */
export function route_target(url: string): string | null {
  const local = {
    geo: process.env.PXL_GEO_BASE_URL || "http://127.0.0.1:3000",
    tile: process.env.PXL_TILE_BASE_URL || "http://127.0.0.1:3001",
    api: process.env.PXL_API_BASE_URL || "http://127.0.0.1:3002",
  };
  const delivery = (process.env.PXL_DEV_SOURCE || "delivery") === "delivery";
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

/** The request forwarded whole and unrewritten. What comes back is never
 * stored: the objects behind it are `immutable` for a year, and a restart of
 * this server cuts a body in flight. */
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
