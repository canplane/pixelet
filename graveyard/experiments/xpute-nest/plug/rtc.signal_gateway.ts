// // @pxl/api-nest/xpute/plug/rtc.signal_gateway.ts

// import {
//   OnGatewayConnection,
//   OnGatewayInit,
//   SubscribeMessage,
//   WebSocketGateway,
//   WebSocketServer,
// } from "@nestjs/websockets";
// import type { Server as IOServer, Socket } from "socket.io";

// import { Vector } from "@xpute/spec/control/vec.ts";
// import type { Handle } from "@xpute/spec/net/xport/mod.ts";
// import {
//   alloc_handle,
//   is_handle_of,
//   TransportKind,
// } from "@xpute/spec/net/xport/mod.ts";
// import type { SignalFrame } from "@xpute/spec/net/mesh.ts";

// import { RTCSignalRelay } from "./rtc.signal_relay.ts";

// @WebSocketGateway({
//   namespace: "/rtc-signal",
//   cors: true,
// })
// export class RTCSignalGateway implements OnGatewayInit, OnGatewayConnection {
//   @WebSocketServer()
//   readonly io!: IOServer;

//   private readonly handle_by_sockid = new Map<string, Handle>();
//   private readonly sockid_by_handle = new Map<Handle, string>();

//   constructor(private readonly relay: RTCSignalRelay) {}

//   afterInit(): void {
//     // Nest injected io server
//   }

//   private stable_handle_of(client: Socket): Handle {
//     const cached = this.handle_by_sockid.get(client.id);
//     if (cached) return cached;

//     // prefer client-provided stable handle (reconnect-safe)
//     const h = (client.handshake as any)?.auth?.mesh_handle;
//     const handle: Handle = is_handle_of(TransportKind.MESH, h)
//       ? h
//       : alloc_handle(TransportKind.MESH);
//     this.handle_by_sockid.set(client.id, handle);
//     return handle;
//   }

//   handleConnection(client: Socket): void {
//     const handle = this.stable_handle_of(client);

//     // detect (first accept) vs (reconnect replacing socket)
//     const prev_sockid = this.sockid_by_handle.get(handle) ?? null;
//     this.sockid_by_handle.set(handle, client.id);

//     if (prev_sockid !== client.id) {
//       // replacement: ensure old socket cleanup
//       if (prev_sockid) {
//         const prev = this.io.sockets.sockets.get(prev_sockid);
//         if (prev) {
//           try {
//             prev.disconnect(true);
//           } catch {}
//         }
//       }
//     }

//     client.on("disconnect", () => {
//       const h = this.handle_by_sockid.get(client.id) ?? handle;
//       this.handle_by_sockid.delete(client.id);

//       // stale disconnect guard
//       const cur = this.sockid_by_handle.get(h);
//       if (cur && cur !== client.id) return;

//       this.sockid_by_handle.delete(h);
//       this.relay.detach_signal_session(h, client);
//     });
//   }

//   @SubscribeMessage("rtc:signal")
//   handleSignal(client: Socket, frame: SignalFrame): void {
//     const handle = this.stable_handle_of(client);

//     // First message in a room: attach session
//     if (frame.vec === Vector.RTC_SIGNAL_JOIN) {
//       this.relay.attach_signal_session(handle, this.io, client, frame.room);
//     }

//     try {
//       this.relay.push_signal_from_client(handle, frame);
//     } catch (err) {
//       // Rate limit / validation errors are already thrown by relay
//       // Socket.io will handle the error (disconnect or ignore)
//       // Silent: already logged in relay
//     }
//   }
// }
