// // @pxl/api-nest/xpute/plug/ws.ts - Server

// import type { Server, Socket } from "socket.io";
// import {
//   OnGatewayConnection,
//   OnGatewayInit,
//   WebSocketGateway,
//   WebSocketServer,
// } from "@nestjs/websockets";
// import { io as ioc, type Socket as ClientSocket } from "socket.io-client";

// import { Errno } from "@xpute/spec/sys/errno.ts";
// import { InvariantError, NetworkFaultError } from "@xpute/spec/sys/error.ts";

// import type { Dial, Handle } from "@xpute/spec/net/xport/mod.ts";
// import {
//   alloc_handle,
//   is_handle_of,
//   TransportKind,
// } from "@xpute/spec/net/xport/mod.ts";
// import type {
//   Message,
//   Wire,
//   WireRx,
//   WireRxIn,
//   WireTx,
//   WireTxIn,
// } from "@xpute/spec/net/central.ts";
// import { errno_from_ws_connect_error } from "@xpute/spec/net/central.ts";
// import { decode_wire_message } from "@xpute/core/net/kit/guard.ts";

// @WebSocketGateway({ path: "/x", cors: true }) // Nest entrypoint, semantic role = Wire plug
// export class WSPlug implements Wire, OnGatewayInit, OnGatewayConnection {
//   @WebSocketServer()
//   readonly server!: Server;

//   private on_rx: ((src: Handle, msg: Message) => void) | null = null;
//   private on_handle_down_cb: ((handle: Handle) => void) | null = null;
//   private on_handle_up_cb: ((handle: Handle) => void) | null = null;
//   private on_handle_dead_cb:
//     | ((handle: Handle, reason?: string) => void)
//     | null = null;
//   private readonly out_socks = new Map<Handle, ClientSocket>(); // outbound links (node==peer)
//   private readonly handle_by_sockid = new Map<string, Handle>(); // inbound: socket.id -> stable handle
//   private readonly sockid_by_handle = new Map<Handle, string>(); // inbound: stable handle -> current socket.id

//   private stable_handle_of(client: Socket): Handle {
//     const cached = this.handle_by_sockid.get(client.id);
//     if (cached) return cached;

//     // prefer client-provided stable handle (reconnect-safe)
//     const h = (client.handshake as any)?.auth?.xpute_handle;
//     const handle: Handle = is_handle_of(TransportKind.CENTRAL, h)
//       ? h
//       : alloc_handle(TransportKind.CENTRAL);
//     this.handle_by_sockid.set(client.id, handle);
//     return handle;
//   }

//   // DIRECT-LINK plug:
//   // - NO relay-by-dst (banned).
//   // - inbound always delivered to local node stack.
//   // - outbound targets a specific transport-owned link handle.
//   //   (accepted inbound sockets are addressed via stable-handle room routing)

//   afterInit(): void {
//     // noop: server injected by Nest
//   }

//   // server node도 peer로서 outbound edge를 열 수 있어야 함.
//   async connect(ws_url: Dial): Promise<Handle> {
//     const handle = alloc_handle(TransportKind.CENTRAL);
//     const sock = ioc(ws_url, {
//       path: "/x",
//       transports: ["websocket"],
//       reconnection: true,
//       reconnectionAttempts: 8,
//       reconnectionDelay: 250,
//       reconnectionDelayMax: 2000,
//       // outbound link handle must be stable across reconnects
//       auth: { xpute_handle: handle },
//     });

//     const rc = await new Promise<Errno>((resolve) => {
//       const on_ok = () => {
//         cleanup();
//         resolve(Errno.OK);
//       };
//       const on_err = (err: any) => {
//         cleanup();
//         resolve(errno_from_ws_connect_error(err));
//       };
//       const cleanup = () => {
//         sock.off("connect", on_ok);
//         sock.off("connect_error", on_err);
//       };
//       sock.on("connect", on_ok);
//       sock.on("connect_error", on_err);
//     });
//     if (rc) {
//       try {
//         sock.close();
//       } catch {}
//       throw new NetworkFaultError(rc, "connect failed");
//     }

//     if (this.out_socks.has(handle)) {
//       try {
//         sock.close();
//       } catch {}
//       throw new InvariantError(Errno.EEXIST, "duplicate handle");
//     }
//     this.out_socks.set(handle, sock);

//     sock.on("disconnect", () => {
//       this.on_handle_down_cb?.(handle);
//       // keep mapping: socket.io may reconnect
//     });

//     const on_up = () => this.on_handle_up_cb?.(handle);
//     sock.on("connect", on_up);
//     on_up();

//     sock.io.on("reconnect_failed", () => {
//       this.on_handle_dead_cb?.(handle, "reconnect_failed");
//       this.out_socks.delete(handle);
//     });

//     sock.on("w", (rx: WireRxIn) => {
//       const msg = decode_wire_message(rx?.msg, "wire/ws/server[out]:w");
//       if (!msg) return;
//       // outbound link inbound: src_handle is our outbound link handle
//       this.on_rx?.(handle, msg);
//     });

//     return handle;
//   }

//   on_handle_down(cb: (handle: Handle) => void): void {
//     this.on_handle_down_cb = cb;
//   }

//   on_handle_up(cb: (handle: Handle) => void): void {
//     this.on_handle_up_cb = cb;
//   }

//   on_handle_dead(cb: (handle: Handle, reason?: string) => void): void {
//     this.on_handle_dead_cb = cb;
//   }

//   handleConnection(client: Socket): void {
//     const handle = this.stable_handle_of(client);
//     // stable handle room: server.to(handle) routes to whichever socket currently holds it.
//     client.join(handle);
//     this.attach(client, handle);

//     // detect (first accept) vs (reconnect replacing socket)
//     const prev_sockid = this.sockid_by_handle.get(handle) ?? null;
//     this.sockid_by_handle.set(handle, client.id);
//     if (prev_sockid !== client.id) {
//       // replacement: ensure old socket stops receiving server.to(handle) broadcasts
//       if (prev_sockid) {
//         const prev = this.server.sockets.sockets.get(prev_sockid);
//         if (prev) {
//           // best-effort: remove from stable-handle room + terminate
//           try {
//             prev.leave(handle);
//           } catch {}
//           try {
//             prev.disconnect(true);
//           } catch {}
//         }
//       }
//       this.on_handle_up_cb?.(handle);
//     }

//     client.on("disconnect", () => {
//       const h = this.handle_by_sockid.get(client.id) ?? handle;
//       this.handle_by_sockid.delete(client.id);

//       // stale disconnect guard:
//       // if this handle already rebound to a new socket.id, ignore old disconnect.
//       const cur = this.sockid_by_handle.get(h);
//       if (cur && cur !== client.id) return;

//       this.sockid_by_handle.delete(h);
//       this.on_handle_down_cb?.(h);
//     });
//   }

//   on(rx: (src: Handle, msg: Message) => void): void {
//     this.on_rx = rx;
//   }

//   /** @throws NetworkFaultError */
//   send(dst: Handle, msg: Message): void {
//     const srv = this.server;
//     if (!srv) {
//       throw new InvariantError(Errno.ENOTCONN, "server not ready");
//     }
//     // dst is a transport-owned Handle.
//     // - outbound dialed sockets: out_socks.get(dst)  (direct handle map)
//     // - inbound accepted sockets: server.to(dst)     (stable-handle room name)
//     const out = this.out_socks.get(dst);
//     if (out) {
//       if (!out.connected) {
//         throw new NetworkFaultError(Errno.ENOTCONN, "socket not connected");
//       }
//       try {
//         out.emit("w", { msg } satisfies WireTx);
//       } catch (err) {
//         throw new NetworkFaultError(Errno.EIO, "emit failed", { cause: err });
//       }
//       return;
//     }
//     // inbound: dst is the stable-handle room name
//     const room = srv.sockets.adapter.rooms.get(dst);
//     if (!room || room.size === 0) {
//       throw new NetworkFaultError(Errno.ENOENT, "no client");
//     }
//     try {
//       srv.to(dst).emit("w", { msg } satisfies WireRx);
//     } catch (err) {
//       throw new NetworkFaultError(Errno.EIO, "emit failed", { cause: err });
//     }
//   }

//   // connection hook
//   private attach(client: Socket, handle: Handle): void {
//     // join socket.id room (socket.io addressing) as a spare/ops-safe path.
//     // not used as the transport Handle; actual plug routing uses the stable-handle room.
//     client.join(client.id);

//     client.on("w", (tx: WireTxIn) => {
//       const msg = decode_wire_message(tx?.msg, "wire/ws/server:w");
//       if (!msg) return;

//       // inbound accepted socket -> local stack, reported as stable transport Handle.
//       const src: Handle = handle;
//       this.on_rx?.(src, msg);
//     });
//   }
// }
