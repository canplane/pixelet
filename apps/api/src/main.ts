// @pxl/api/main.ts

// TODO: i18n initialization (not wired up yet)

import { Hono } from "@hono/hono";
import { bodyLimit } from "@hono/hono/body-limit";
import { createServer, type IncomingMessage, type ServerResponse } from "node:http";
import { Readable } from "node:stream";
import { Server as SocketIoServer } from "socket.io";

import { type AppResources, make_app_resrcs } from "@/app/resources.ts";
import { mount_app } from "@/app/mount.ts";
import { type AppConfig, make_cfg } from "@/app/config.ts";

export type ServerContext = {
  server: ReturnType<typeof createServer>; // Node HTTP server (owns the listening port and request lifecycle)
  router: Hono; // HTTP router/app (routes requests; mounted behind server via fetch adapter)

  io: SocketIoServer; // Socket.IO server (real-time transport bound to the same server)
};

function make_router(cfg: AppConfig): Hono {
  const router = new Hono();
  router.use("/api/*", bodyLimit({ maxSize: cfg.max_body_sz }));
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

      // node -> web stream (best-effort, 1-liner)
      const body: any = method === "GET" || method === "HEAD" ? null : ((Readable as any).toWeb?.(req) ?? req);

      const resp = await router.fetch(new Request(url, { method, headers, body }));

      res.statusCode = resp.status;
      resp.headers.forEach((v, k) => res.setHeader(k, v));

      if (!resp.body) {
        res.end();
        return;
      }

      // web -> node stream (best-effort, 1-liner + fallback)
      const node_stream: any = (Readable as any).fromWeb?.(resp.body);
      if (node_stream) return void node_stream.pipe(res);
      res.end(new Uint8Array(await resp.arrayBuffer()));
    } catch {
      // last resort
      try {
        res.statusCode = 500;
        res.end();
      } catch {}
    }
  };
}

function make_listen(cfg: AppConfig, server: ReturnType<typeof createServer>) {
  return function listen(): void {
    server.listen(cfg.port, cfg.host, () => {
      console.log(`app: listening on ${cfg.host}:${cfg.port} (ws: ${cfg.socket_path})`);
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
    try {
      await resrc?.dispose();
    } catch {}
    try {
      io.close();
    } catch {}
    try {
      server.close();
    } catch {}
    throw err;
  }
}
