// @pxl/web/xpute/plug/ws.ts - Browser

import { io, type Socket } from "socket.io-client";

import { Errno } from "@xpute/spec/sys/errno.ts";
import { InvariantError, NetworkFaultError } from "@xpute/spec/sys/error.ts";

import type { Dial, Handle } from "@xpute/spec/net/xport/mod.ts";
import { alloc_handle, TransportKind } from "@xpute/spec/net/xport/mod.ts";
import type { Message, Wire, WireRxIn, WireTx } from "@xpute/spec/net/xport/central.ts";
import { errno_from_ws_connect_error } from "@xpute/spec/net/xport/central.ts";
import { decode_wire_message } from "@xpute/core/net/kit/guard.ts";

export class WSPlug implements Wire {
  private readonly socks = new Map<Handle, Socket>();
  private on_rx: ((src: Handle, msg: Message) => void) | null = null;
  private on_handle_down_cb: ((handle: Handle) => void) | null = null;
  private on_handle_up_cb: ((handle: Handle) => void) | null = null;
  private on_handle_dead_cb: ((handle: Handle, reason?: string) => void) | null = null;

  attach(): void {
    // browser ws plug has no server to attach to; keep as a semantic hook
  }

  detach(): void {
    for (const [, sock] of this.socks) {
      try {
        sock.removeAllListeners();
      } catch {}
      try {
        sock.close();
      } catch {}
    }
    this.socks.clear();
    this.on_rx = null;
    this.on_handle_down_cb = null;
    this.on_handle_up_cb = null;
    this.on_handle_dead_cb = null;
  }

  /**
   * Handle is transport-owned link identifier (stable across socket.io reconnects).
   * socket.id may churn; we do NOT use it as Handle.
   *
   * @throws NetworkFaultError
   */
  async connect(ws_url: Dial): Promise<Handle> {
    const handle: Handle = alloc_handle(TransportKind.CENTRAL);
    const sock = io(ws_url, {
      path: "/io",
      transports: ["websocket"],
      // socket.io 기본 재접속 로직 사용 (컨벤션에 의존)
      reconnection: true,
      reconnectionAttempts: 8,
      reconnectionDelay: 250,
      reconnectionDelayMax: 2000,
      auth: { xpute_handle: handle },
    });

    let rc: Errno;

    rc = await new Promise<Errno>((resolve) => {
      const on_ok = () => {
        cleanup();
        resolve(Errno.OK);
      };
      const on_err = (err: any) => {
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

    if (this.socks.has(handle)) {
      try {
        sock.close();
      } catch {}
      throw new InvariantError(Errno.EEXIST, "duplicate handle");
    }
    this.socks.set(handle, sock);

    sock.on("disconnect", () => {
      this.on_handle_down_cb?.(handle);
      // keep mapping: socket.io may reconnect with same Socket instance.
    });

    // "up" == connected (initial connect + reconnect)
    const on_up = () => this.on_handle_up_cb?.(handle);
    sock.on("connect", on_up);
    on_up();

    // "dead" == socket.io가 재접속 시도 후 최종 포기
    sock.io.on("reconnect_failed", () => {
      this.on_handle_dead_cb?.(handle, "reconnect_failed");
      this.socks.delete(handle); // terminal: no more reconnect on this Socket
    });

    sock.on("w", (rx: WireRxIn) => {
      // inbound src_handle is the transport-owned stable link handle for this socket.
      // (socket.io socket.id may churn across reconnects; we do not expose it as Handle)
      const msg = decode_wire_message(rx?.msg, "wire/ws/client:w");
      if (!msg) return;
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

  on(rx: (src: Handle, msg: Message) => void): void {
    this.on_rx = rx;
  }

  /** @throws NetworkFaultError */
  send(dst: Handle, msg: Message): void {
    // dst is the link-handle selecting the socket instance (1 peer node == 1 ws handle).
    const sock = this.socks.get(dst);
    if (!sock) throw new NetworkFaultError(Errno.ENOENT, "unknown handle");
    if (!sock.connected) throw new NetworkFaultError(Errno.ENOTCONN, "socket not connected");
    try {
      sock.emit("w", { msg } satisfies WireTx);
    } catch (err) {
      throw new NetworkFaultError(Errno.EIO, "emit failed", { cause: err });
    }
  }
}
