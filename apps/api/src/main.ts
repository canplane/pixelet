// @pxl/api/main.ts

// TODO: i18n initialization (not wired up yet)

import { Hono } from "@hono/hono";
import { bodyLimit } from "@hono/hono/body-limit";
import { compress } from "@hono/hono/compress";
import { createServer, type IncomingMessage, type ServerResponse } from "node:http";
import { Readable } from "node:stream";
import { Server as SocketIoServer } from "socket.io";

import { type AppResources, make_app_resrcs } from "@/app/resource.ts";
import { mount_app } from "@/app/mount.ts";
import { type AppConfig, make_cfg } from "@/app/config.ts";
import { admin_auth } from "@/lib/admin_auth.ts";

export type ServerContext = {
  server: ReturnType<typeof createServer>; // Node HTTP server (owns the listening port and request lifecycle)
  router: Hono; // HTTP router/app (routes requests; mounted behind server via fetch adapter)

  io: SocketIoServer; // Socket.IO server (real-time transport bound to the same server)
};

function make_router(cfg: AppConfig): Hono {
  const router = new Hono();
  router.use("/api/*", bodyLimit({ maxSize: cfg.max_body_sz }));
  // terrain tile responses are dense per-cell ground-channel bytes (~256KB/page,
  // see format.ts's encode_terrain) built mostly from large uniform runs
  // (polygon fills) — highly compressible, and gzip here is transparent to
  // fetch() on the client (Content-Encoding is auto-decoded), no wire-format
  // or client change needed.
  router.use("/api/*", compress());
  // ops-only routes (cache invalidation, etc.) — everything else under
  // /api/* stays public, this prefix requires the shared admin token.
  router.use("/api/internal/*", admin_auth(cfg.admin_token));
  return router;
}

function make_node_http_handler(cfg: AppConfig, router: Hono) {
  // node http handler: IncomingMessage -> Request -> Hono.fetch -> ServerResponse
  return async function handle_node_request(req: IncomingMessage, res: ServerResponse): Promise<void> {
    try {
      const url = new URL(req.url ?? "/", `http://${cfg.host}:${cfg.port}`);
      const method = req.method ?? "GET";

      const headers = new Headers();
      for (const [k, v] of Object.entries(req.headers)) {
        if (v == null) continue;
        if (Array.isArray(v)) headers.set(k, v.join(", "));
        else headers.set(k, v);
      }

      // node -> web stream (best-effort, 1-liner). Request's own BodyInit type
      // has no overload for a Node Readable/ReadableStream, so this bridge is
      // untyped on both ends regardless of how the cast is spelled.
      // deno-lint-ignore no-explicit-any
      const body: any = method === "GET" || method === "HEAD" ? null : (Readable.toWeb?.(req) ?? req);

      const resp = await router.fetch(new Request(url, { method, headers, body }));

      res.statusCode = resp.status;
      resp.headers.forEach((v, k) => res.setHeader(k, v));

      if (!resp.body) {
        res.end();
        return;
      }

      // web -> node stream (best-effort, 1-liner + fallback). Node's
      // stream/web ReadableStream and the DOM ReadableStream Response.body
      // returns aren't nominally the same type, despite matching at runtime.
      // deno-lint-ignore no-explicit-any
      const node_stream: any = Readable.fromWeb?.(resp.body as any);
      if (node_stream) return void node_stream.pipe(res);
      res.end(new Uint8Array(await resp.arrayBuffer()));
    } catch {
      // last resort
      try {
        res.statusCode = 500;
        res.end();
      } catch {
        // last resort — e.g. headers already sent; nothing left to do
      }
    }
  };
}

function make_listen(cfg: AppConfig, server: ReturnType<typeof createServer>) {
  return function listen(): void {
    server.listen(cfg.port, cfg.host, () => {
      console.log(`[app] listening on ${cfg.host}:${cfg.port} (ws: ${cfg.socket_path})`);
    });
  };
}

export async function main(): Promise<void> {
  // ---- cfg ----
  const cfg = make_cfg();

  // ---- hono ----
  const router: Hono = make_router(cfg);

  // ---- node http ----
  const handle_node_request = make_node_http_handler(cfg, router);
  const server = createServer((req, res) => void handle_node_request(req, res));

  // ---- socket.io ----
  const io = new SocketIoServer(server, {
    path: cfg.socket_path,
    transports: ["websocket"],
  });

  // ---- wiring ----
  const ctx: ServerContext = { server, router, io };

  let resrc: AppResources | null = null;

  try {
    // ---- app resources ----
    resrc = await make_app_resrcs();
    mount_app(ctx, resrc);

    // ---- online ----
    const listen = make_listen(cfg, server);
    listen();
  } catch (err) {
    // Startup failed — unwind whatever came up before rethrowing err. Each
    // step is independent: one failing (e.g. dispose() on a pool that never
    // finished connecting) must not skip the others.
    try {
      await resrc?.dispose();
    } catch {
      // dispose() itself failing has no fallback — err below is the real signal
    }
    try {
      io.close();
    } catch {
      // io was never listening yet in most failure paths; close() on that is a no-op that can still throw
    }
    try {
      server.close();
    } catch {
      // server was never listening yet in most failure paths; close() on that is a no-op that can still throw
    }
    throw err;
  }
}
