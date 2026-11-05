// @pxl/client/script/proxy.ts

/**
 * What a dev server and the page's preview (script/web.ts `serve`) answer
 * besides the bundle: the routes the page asks of its own origin, forwarded
 * where the tiles come from, and a device's console. Loaded by Expo's
 * transform for metro.config.ts as well as by Node, so it reads no
 * `import.meta`.
 */

import * as fs from "node:fs";
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

/** What a device's console posts to `/__console` (src/remote-console.ts),
 * printed where this server's output goes and appended to `log`, so a phone
 * on a tunnel can be read without a debugger attached. */
export function console_post(
  log: string,
  req: http.IncomingMessage,
  res: http.ServerResponse,
): void {
  if (req.method !== "POST") {
    res.statusCode = 405;
    res.end();
    return;
  }
  let body = "";
  req.on("data", (c: Buffer) => {
    if (body.length < 1 << 16) body += c.toString();
  });
  req.on("end", () => {
    const lines = body.split("\n").filter(Boolean);
    for (const line of lines) console.log(line);
    if (lines.length > 0) {
      fs.appendFileSync(
        log,
        lines.map((l) => `${new Date().toISOString()} ${l}\n`).join(""),
      );
    }
    res.statusCode = 204;
    res.end();
  });
}

export type Middleware = (
  req: http.IncomingMessage,
  res: http.ServerResponse,
  next: () => void,
) => void;
