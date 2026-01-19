// @pxl/api/xpute/plug/ws.ts - Server

import type { Server as SocketIoServer, Socket } from "socket.io";
import { io as ioc, type Socket as ClientSocket } from "socket.io-client";

import { Errno } from "@xpute/spec/sys/errno.ts";
import { InvariantError, NetworkFaultError } from "@xpute/spec/sys/error.ts";

import { Vector } from "@xpute/spec/control/vec.ts";
import { sniff_wire_vec } from "@xpute/spec/rpc/wire_header.ts";
import type { Dial, Handle } from "@xpute/spec/net/xport/mod.ts";
import { alloc_handle, is_handle_of, TransportKind } from "@xpute/spec/net/xport/mod.ts";
import type { Message, Wire, WireRx, WireRxIn, WireTx, WireTxIn } from "@xpute/spec/net/xport/central.ts";
import { errno_from_ws_connect_error } from "@xpute/spec/net/xport/central.ts";
import { decode_wire_message } from "@xpute/core/net/kit/guard.ts";
import { drop } from "@xpute/core/net/kit/drop.ts";

export class WSPlug implements Wire {
  private io: SocketIoServer | null = null;

  private on_rx: ((src: Handle, msg: Message) => void) | null = null;
  private on_handle_down_cb: ((handle: Handle) => void) | null = null;
  private on_handle_up_cb: ((handle: Handle) => void) | null = null;
  private on_handle_dead_cb: ((handle: Handle, reason?: string) => void) | null = null;
  private readonly out_socks = new Map<Handle, ClientSocket>(); // outbound links (node==peer)
  private readonly handle_by_sockid = new Map<string, Handle>(); // inbound: socket.id -> stable handle
  private readonly sockid_by_handle = new Map<Handle, string>(); // inbound: stable handle -> current socket.id
  private readonly src_by_handle = new Map<Handle, string>(); // stable handle -> src_hint

  // HELLO pre-auth per-src RL (server-side only)
  private readonly hello_rl_by_src = new Map<string, { ts: number; n: number }>();
  private readonly hello_rl_window_ms = 1000;
  private readonly hello_rl_burst = 200; // N per window (tune per env)
  private readonly hello_rl_ttl_ms = 10_000; // stale src entry TTL (>= window)
  private readonly hello_rl_max = 8192; // hard cap to prevent map DoS
  private hello_rl_last_sweep_ms = 0;

  // src attribution trust boundary (plug policy)
  // - 0: ignore x-forwarded-for (default; safe)
  // - 1: trust x-forwarded-for (only behind trusted proxy)
  private readonly trust_proxy_xff: 0 | 1 = 0;

  /** Attach the socket.io Server. Must be called before use. */
  attach(io: SocketIoServer): void {
    this.io = io;
    io.on("connection", (client) => this.handleConnection(client));
  }

  // best-effort teardown (container shutdown hook)
  detach(): void {
    for (const [, sock] of this.out_socks) {
      try {
        sock.removeAllListeners();
      } catch {}
      try {
        sock.close();
      } catch {}
    }
    this.out_socks.clear();
    this.handle_by_sockid.clear();
    this.sockid_by_handle.clear();
    this.src_by_handle.clear();
    this.hello_rl_by_src.clear();
    this.io = null;
  }

  private hello_sweep(now: number): void {
    if (now - this.hello_rl_last_sweep_ms < this.hello_rl_window_ms) return;
    this.hello_rl_last_sweep_ms = now;

    // TTL prune
    for (const [k, st] of this.hello_rl_by_src) {
      if (now - st.ts > this.hello_rl_ttl_ms) this.hello_rl_by_src.delete(k);
    }

    // hard cap: if still too big, drop oldest-ish by ts (O(N), rare)
    if (this.hello_rl_by_src.size <= this.hello_rl_max) return;

    const over = this.hello_rl_by_src.size - this.hello_rl_max;
    const arr: Array<[string, number]> = [];
    for (const [k, st] of this.hello_rl_by_src) arr.push([k, st.ts]);
    arr.sort((a, b) => a[1] - b[1]);
    for (let i = 0; i < over && i < arr.length; i++) this.hello_rl_by_src.delete(arr[i][0]);
  }

  private src_hint_of(client: Socket): string {
    const h: any = client.handshake ?? {};
    const headers: any = h.headers ?? {};

    // prefer proxy-provided xff if present (policy: plug-level trust boundary)
    let ip = "";
    if (this.trust_proxy_xff) {
      const xff = typeof headers["x-forwarded-for"] === "string" ? headers["x-forwarded-for"] : "";
      if (xff) ip = xff.split(",")[0]?.trim() ?? "";
    }

    if (!ip) {
      const addr = typeof h.address === "string" ? h.address : "";
      ip = addr || (typeof (client as any).conn?.remoteAddress === "string" ? (client as any).conn.remoteAddress : "") || "";
    }

    const ua = typeof headers["user-agent"] === "string" ? headers["user-agent"] : "";
    if (!ip) return "";
    return `${ip}|${ua}`;
  }

  private hello_allow_src(src: string): boolean {
    const now = Date.now();
    this.hello_sweep(now);
    if (this.hello_rl_by_src.size >= this.hello_rl_max) return false;
    let st = this.hello_rl_by_src.get(src);
    if (!st || now - st.ts >= this.hello_rl_window_ms) {
      st = { ts: now, n: 0 };
      this.hello_rl_by_src.set(src, st);
    }
    st.n++;
    return st.n <= this.hello_rl_burst;
  }

  private stable_handle_of(client: Socket): Handle {
    const cached = this.handle_by_sockid.get(client.id);
    if (cached) return cached;

    // prefer client-provided stable handle (reconnect-safe)
    const h = client.handshake?.auth?.xpute_handle;
    const handle: Handle = is_handle_of(TransportKind.CENTRAL, h) ? h : alloc_handle(TransportKind.CENTRAL);
    this.src_by_handle.set(handle, this.src_hint_of(client));
    this.handle_by_sockid.set(client.id, handle);
    return handle;
  }

  // DIRECT-LINK plug:
  // - NO relay-by-dst (banned).
  // - inbound always delivered to local node stack.
  // - outbound targets a specific transport-owned link handle.
  //   (accepted inbound sockets are addressed via stable-handle room routing)

  // server node도 peer로서 outbound edge를 열 수 있어야 함.
  async connect(ws_url: Dial): Promise<Handle> {
    const handle = alloc_handle(TransportKind.CENTRAL);
    const sock = ioc(ws_url, {
      path: "/io",
      transports: ["websocket"],
      reconnection: true,
      reconnectionAttempts: 8,
      reconnectionDelay: 250,
      reconnectionDelayMax: 2000,
      // outbound link handle must be stable across reconnects
      auth: { xpute_handle: handle },
    });

    const rc = await new Promise<Errno>((resolve) => {
      const on_ok = () => {
        cleanup();
        resolve(Errno.OK);
      };
      const on_err = (err: unknown) => {
        cleanup();
        resolve(errno_from_ws_connect_error(err));
      };
      const cleanup = () => {
        sock.off("connect", on_ok);
        sock.off("connect_error", on_err);
      };
      sock.on("connect", on_ok);
      sock.on("connect_error", on_err);
    });
    if (rc) {
      try {
        sock.close();
      } catch {}
      throw new NetworkFaultError(rc, "connect failed");
    }

    if (this.out_socks.has(handle)) {
      try {
        sock.close();
      } catch {}
      throw new InvariantError(Errno.EEXIST, "duplicate handle");
    }
    this.out_socks.set(handle, sock);

    sock.on("disconnect", () => {
      this.on_handle_down_cb?.(handle);
      // keep mapping: socket.io may reconnect
    });

    const on_up = () => this.on_handle_up_cb?.(handle);
    sock.on("connect", on_up);
    on_up();

    sock.io.on("reconnect_failed", () => {
      this.on_handle_dead_cb?.(handle, "reconnect_failed");
      this.out_socks.delete(handle);
    });

    sock.on("w", (rx: WireRxIn) => {
      const msg = decode_wire_message(rx?.msg, "wire/ws/server[out]:w");
      if (!msg) return;
      // outbound link inbound: src_handle is our outbound link handle
      this.on_rx?.(handle, msg);
    });

    return handle;
  }

  on_handle_down(cb: (handle: Handle) => void): void {
    this.on_handle_down_cb = cb;
  }

  on_handle_up(cb: (handle: Handle) => void): void {
    this.on_handle_up_cb = cb;
  }

  on_handle_dead(cb: (handle: Handle, reason?: string) => void): void {
    this.on_handle_dead_cb = cb;
  }

  private handleConnection(client: Socket): void {
    if (!this.io) {
      // should be impossible if attach() is used correctly
      throw new InvariantError(Errno.ENOTCONN, "server not ready");
    }
    const handle = this.stable_handle_of(client);
    // stable handle room: io.to(handle) routes to whichever socket currently holds it.
    client.join(handle);
    this.attach_client(client, handle);

    // detect (first accept) vs (reconnect replacing socket)
    const prev_sockid = this.sockid_by_handle.get(handle) ?? null;
    this.sockid_by_handle.set(handle, client.id);
    if (prev_sockid !== client.id) {
      // replacement: ensure old socket stops receiving io.to(handle) broadcasts
      if (prev_sockid) {
        const prev = this.io.sockets.sockets.get(prev_sockid);
        if (prev) {
          // best-effort: remove from stable-handle room + terminate
          try {
            prev.leave(handle);
          } catch {}
          try {
            prev.disconnect(true);
          } catch {}
        }
      }
      this.on_handle_up_cb?.(handle);
    }

    client.on("disconnect", () => {
      const h = this.handle_by_sockid.get(client.id) ?? handle;
      this.handle_by_sockid.delete(client.id);

      // stale disconnect guard:
      // if this handle already rebound to a new socket.id, ignore old disconnect.
      const cur = this.sockid_by_handle.get(h);
      if (cur && cur !== client.id) return;

      this.sockid_by_handle.delete(h);
      this.src_by_handle.delete(h);
      this.on_handle_down_cb?.(h);
    });
  }

  on(rx: (src: Handle, msg: Message) => void): void {
    this.on_rx = rx;
  }

  /** @throws NetworkFaultError */
  send(dst: Handle, msg: Message): void {
    const io = this.io;
    if (!io) {
      throw new InvariantError(Errno.ENOTCONN, "server not ready");
    }
    // dst is a transport-owned Handle.
    // - outbound dialed sockets: out_socks.get(dst)  (direct handle map)
    // - inbound accepted sockets: server.to(dst)     (stable-handle room name)
    const out = this.out_socks.get(dst);
    if (out) {
      if (!out.connected) throw new NetworkFaultError(Errno.ENOTCONN, "socket not connected");
      try {
        out.emit("w", { msg } satisfies WireTx);
      } catch (err) {
        throw new NetworkFaultError(Errno.EIO, "emit failed", { cause: err });
      }
      return;
    }
    // inbound: dst is the stable-handle room name
    const room = io.sockets.adapter.rooms.get(dst);
    if (!room || room.size === 0) throw new NetworkFaultError(Errno.ENOENT, "no client");
    try {
      io.to(dst).emit("w", { msg } satisfies WireRx);
    } catch (err) {
      throw new NetworkFaultError(Errno.EIO, "emit failed", { cause: err });
    }
  }

  // connection hook
  private attach_client(client: Socket, handle: Handle): void {
    // join socket.id room (socket.io addressing) as a spare/ops-safe path.
    // not used as the transport Handle; actual plug routing uses the stable-handle room.
    client.join(client.id);

    client.on("w", (tx: WireTxIn) => {
      const msg = decode_wire_message(tx?.msg, "wire/ws/server:w");
      if (!msg) return;

      // HELLO-only per-src RL (pre-auth hot path)
      const vec = sniff_wire_vec(msg.enc_hdr);
      if (vec === Vector.NET_HELLO) {
        const src = this.src_by_handle.get(handle) ?? "";
        if (src) {
          if (!this.hello_allow_src(src)) {
            drop("ws_wire", { why: "hello_rate_limited" });
            return;
          }
        }
      }

      // inbound accepted socket -> local stack, reported as stable transport Handle.
      const src: Handle = handle;
      this.on_rx?.(src, msg);
    });
  }
}
