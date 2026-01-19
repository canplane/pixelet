// @pxl/api/xpute/plug/rtc.signal_gateway.ts

import type { Namespace, Server as SocketIoServer, Socket } from "socket.io";

import { Vector } from "@xpute/spec/control/vec.ts";
import type { Handle } from "@xpute/spec/net/xport/mod.ts";
import { alloc_handle, is_handle_of, TransportKind } from "@xpute/spec/net/xport/mod.ts";
import type { SignalFrame } from "@xpute/spec/net/xport/mesh.ts";
import { drop } from "@xpute/core/net/kit/drop.ts";

import { RTCSignalRelay } from "./rtc.signal_relay.ts";

export class RTCSignalGateway {
  private ns: Namespace | null = null;
  private on_conn: ((client: Socket) => void) | null = null;

  private readonly handle_by_sockid = new Map<string, Handle>();
  private readonly sockid_by_handle = new Map<Handle, string>();

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

  constructor(private readonly relay: RTCSignalRelay) {}

  attach(io: SocketIoServer): void {
    const ns = io.of("/rtc-signal") as any as Namespace;
    this.ns = ns;
    const on_conn = (client: Socket) => this.handleConnection(client);
    this.on_conn = on_conn;
    ns.on("connection", on_conn);
  }

  // best-effort detach (container shutdown hook)
  detach(): void {
    const ns = this.ns;
    const on_conn = this.on_conn;
    this.on_conn = null;
    this.ns = null;
    if (ns && on_conn) {
      try {
        ns.off("connection", on_conn);
      } catch {}
    }
    this.hello_rl_by_src.clear();
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
    arr.sort((a, b) => a[1] - b[1]); // oldest first
    for (let i = 0; i < over && i < arr.length; i++) this.hello_rl_by_src.delete(arr[i][0]);
  }

  private src_hint_of(client: Socket): string {
    const h: any = client.handshake ?? {};
    const headers: any = h.headers ?? {};

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
    const h = client.handshake?.auth?.mesh_handle;
    const handle: Handle = is_handle_of(TransportKind.MESH, h) ? h : alloc_handle(TransportKind.MESH); // fallback to alloc

    this.handle_by_sockid.set(client.id, handle);
    return handle;
  }

  private handleConnection(client: Socket): void {
    const handle = this.stable_handle_of(client);

    // detect (first accept) vs (reconnect replacing socket)
    const prev_sockid = this.sockid_by_handle.get(handle) ?? null;
    this.sockid_by_handle.set(handle, client.id);

    if (prev_sockid !== client.id) {
      // replacement: ensure old socket cleanup
      if (prev_sockid && this.ns) {
        const prev = this.ns.sockets.get(prev_sockid);
        if (prev) {
          try {
            this.relay.detach_signal_session(handle, prev);
          } catch {}
          try {
            prev.disconnect(true);
          } catch {}
        }
      }
    }

    client.on("disconnect", () => {
      const h = this.handle_by_sockid.get(client.id) ?? handle;
      this.handle_by_sockid.delete(client.id);

      // stale disconnect guard
      const cur = this.sockid_by_handle.get(h);
      if (cur && cur !== client.id) return;

      this.sockid_by_handle.delete(h);
      this.relay.detach_signal_session(h, client);
    });

    client.on("rtc:signal", (frame: SignalFrame) => this.handleSignal(client, frame));
  }

  private handleSignal(client: Socket, frame: SignalFrame): void {
    const handle = this.stable_handle_of(client);

    // First message in a room: attach session
    if (frame.vec === Vector.RTC_SIGNAL_JOIN && this.ns) {
      this.relay.attach_signal_session(handle, client, frame.room);
    }

    // HELLO-only per-src RL (pre-auth hot path)
    if (frame.vec === Vector.NET_HELLO) {
      const src = this.src_hint_of(client);
      if (src) {
        if (!this.hello_allow_src(src)) {
          drop("rtc_signal", { why: "hello_rate_limited" });
          return;
        }
      }
    }

    try {
      this.relay.push_signal_from_client(handle, frame);
    } catch {
      // Rate limit / validation errors are already thrown by relay
      // Silent: already logged in relay
    }
  }
}
