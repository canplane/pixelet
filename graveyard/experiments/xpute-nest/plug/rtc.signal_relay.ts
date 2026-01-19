// // @pxl/api-nest/xpute/plug/rtc.signal_relay.ts

// import { Injectable } from "@nestjs/common";
// import type { Server as IOServer, Socket } from "socket.io";

// import type { Bytes } from "@xpute/spec/lib/kit/array.ts";
// import type { Signature } from "@xpute/spec/lib/kit/crypto.ts";
// import { Errno } from "@xpute/spec/sys/errno.ts";
// import { NetworkFaultError } from "@xpute/spec/sys/error.ts";
// import * as encoding from "@xpute/spec/lib/kit/encoding.ts";

// import { Vector } from "@xpute/spec/control/vec.ts";
// import type { RID } from "@xpute/spec/rpc/rid.ts";
// import type { Handle, TransportEvent } from "@xpute/spec/net/xport/mod.ts";
// import type {
//   MeshAdvert,
//   MeshDialSeed,
//   MeshPlug,
//   SignalFrame,
// } from "@xpute/spec/net/mesh.ts";

// const SIGNAL_RATE_MS = 50;

// type SignalSess = {
//   io: IOServer;
//   sock: Socket;
//   room: string;
// };

// type State = {
//   sig_sess: SignalSess | null;
//   sig_room: string | null;
//   closed: 0 | 1;
//   last_signal_ts: number;
// };

// @Injectable()
// export class RTCSignalRelay implements MeshPlug {
//   private hello_cb:
//     | ((handle: Handle, enc_hdr: Bytes, sig: Signature) => void)
//     | null = null;
//   private ev_cb: ((ev: TransportEvent) => void) | null = null;
//   private packet_cb:
//     | ((
//       handle: Handle,
//       rid: RID,
//       enc_hdr: Bytes,
//       enc_body: Bytes | null,
//       sig: Signature,
//     ) => void)
//     | null = null;

//   private readonly st_by_handle = new Map<Handle, State>();
//   private readonly handles_by_room = new Map<string, Set<Handle>>();

//   private room_add(room: string, handle: Handle): void {
//     let set = this.handles_by_room.get(room);
//     if (!set) {
//       set = new Set();
//       this.handles_by_room.set(room, set);
//     }
//     set.add(handle);
//   }

//   private room_del(room: string, handle: Handle): void {
//     let set = this.handles_by_room.get(room);
//     if (!set) return;
//     set.delete(handle);
//     if (!set.size) this.handles_by_room.delete(room);
//   }

//   private st_of(handle: Handle): State {
//     let st = this.st_by_handle.get(handle);
//     if (st) return st;

//     st = {
//       sig_sess: null,
//       sig_room: null,
//       closed: 0,
//       last_signal_ts: 0,
//     };
//     this.st_by_handle.set(handle, st);
//     return st;
//   }

//   private ensure_open_handle(handle: Handle): State {
//     const st = this.st_by_handle.get(handle);
//     if (!st || st.closed) {
//       throw new NetworkFaultError(Errno.ENOENT, "mesh handle missing");
//     }
//     return st;
//   }

//   private emit_event(ev: TransportEvent): void {
//     this.ev_cb?.(ev);
//   }

//   private signal_emit(st: State, handle: Handle, msg: SignalFrame): void {
//     let sess = st.sig_sess;
//     if (!sess) {
//       throw new NetworkFaultError(
//         Errno.ENOTCONN,
//         "mesh signaling session missing",
//       );
//     }
//     sess.sock.to(sess.room).emit("rtc:signal", msg);
//   }

//   // ============ MeshPlug implementation ============

//   async seed(handle: Handle, dial: MeshDialSeed): Promise<void> {
//     // Server doesn't pre-seed - client provides handle in join
//   }

//   async send_hello_bootstrap(
//     handle: Handle,
//     enc_hdr: Bytes,
//     sig: Signature,
//   ): Promise<void> {
//     let st = this.ensure_open_handle(handle);
//     if (!st.sig_room) {
//       throw new NetworkFaultError(
//         Errno.ENOTCONN,
//         "mesh signaling room not prepared",
//       );
//     }

//     const now = Date.now();
//     if (now - st.last_signal_ts < SIGNAL_RATE_MS) {
//       throw new NetworkFaultError(Errno.EAGAIN, "signal rate limit");
//     }
//     st.last_signal_ts = now;

//     this.signal_emit(st, handle, {
//       vec: Vector.NET_HELLO,
//       handle,
//       room: st.sig_room,
//       enc_hdr_b64url: encoding.bytes_to_b64url(enc_hdr),
//       sig_b64url: encoding.bytes_to_b64url(sig),
//     });
//   }

//   async send_packet(
//     handle: Handle,
//     rid: RID,
//     enc_hdr: Bytes,
//     enc_body: Bytes | null,
//     sig: Signature,
//   ): Promise<void> {
//     throw new NetworkFaultError(
//       Errno.ENOTSUP,
//       "server mesh relay has no datachannel",
//     );
//   }

//   apply_advert(handle: Handle, advert?: MeshAdvert): void {
//     // server relay ignores ICE hints (browser-only)
//   }

//   on_hello_bootstrap(
//     cb: (handle: Handle, enc_hdr: Bytes, sig: Signature) => void,
//   ): void {
//     this.hello_cb = cb;
//   }

//   on_event(cb: (ev: TransportEvent) => void): void {
//     this.ev_cb = cb;
//   }

//   on_packet(
//     cb: (
//       handle: Handle,
//       rid: RID,
//       enc_hdr: Bytes,
//       enc_body: Bytes | null,
//       sig: Signature,
//     ) => void,
//   ): void {
//     this.packet_cb = cb;
//     // Server has no DataChannel, so this callback is never invoked
//     // But MeshTransport still registers it (interface requirement)
//   }

//   async close_handle(handle: Handle): Promise<void> {
//     const st = this.st_by_handle.get(handle);
//     if (!st) return;

//     st.closed = 1;
//     if (st.sig_room) this.room_del(st.sig_room, handle);
//     st.sig_sess = null;
//     st.sig_room = null;

//     this.st_by_handle.delete(handle);
//   }

//   // ============ Signaling integration (Gateway) ============

//   attach_signal_session(
//     handle: Handle,
//     io: IOServer,
//     sock: Socket,
//     room: string,
//   ): void {
//     let st = this.st_of(handle);

//     // Verify room matches seed() if already set
//     if (st.sig_room && st.sig_room !== room) {
//       throw new NetworkFaultError(
//         Errno.EINVAL,
//         `room mismatch: seed=${st.sig_room} join=${room}`,
//       );
//     }

//     st.sig_sess = { io, sock, room };
//     st.sig_room = room;
//     st.closed = 0;

//     try {
//       sock.join(room);
//     } catch {}
//     this.room_add(room, handle);
//   }

//   detach_signal_session(handle: Handle, sock?: Socket): void {
//     let st = this.st_by_handle.get(handle);
//     if (!st) return;
//     if (sock && st.sig_sess && st.sig_sess.sock.id !== sock.id) return;
//     if (st.sig_sess) {
//       try {
//         st.sig_sess.sock.to(st.sig_sess.room).emit(
//           "rtc:signal",
//           {
//             vec: Vector.RTC_SIGNAL_PEER_LEFT,
//             handle,
//             room: st.sig_sess.room,
//           } satisfies SignalFrame,
//         );
//       } catch {}
//       try {
//         st.sig_sess.sock.leave(st.sig_sess.room);
//       } catch {}
//       this.room_del(st.sig_sess.room, handle);
//     }
//     st.sig_sess = null;
//     this.emit_event({ vec: Vector.LINK_DOWN, handle, reason: "signal_detach" });
//   }

//   push_signal_from_client(handle: Handle, frame: SignalFrame): void {
//     let st = this.ensure_open_handle(handle);
//     if (!st.sig_room || frame.room !== st.sig_room) {
//       throw new NetworkFaultError(Errno.EINVAL, "signal room mismatch");
//     }

//     if (frame.vec === Vector.RTC_SIGNAL_JOIN) {
//       const sess = st.sig_sess;
//       if (sess) {
//         const set = this.handles_by_room.get(st.sig_room);
//         if (set) {
//           for (const h of set) {
//             if (h === handle) continue;
//             try {
//               sess.sock.emit(
//                 "rtc:signal",
//                 {
//                   vec: Vector.RTC_SIGNAL_PEER_JOINED,
//                   handle: h,
//                   room: st.sig_room,
//                   polite: 0,
//                 } satisfies SignalFrame,
//               );
//             } catch {}
//           }
//         }
//       }

//       try {
//         this.signal_emit(st, handle, {
//           vec: Vector.RTC_SIGNAL_PEER_JOINED,
//           handle,
//           room: st.sig_room,
//           polite: frame.polite,
//         });
//       } catch {
//         throw new NetworkFaultError(
//           Errno.ENETUNREACH,
//           "signal join relay failed",
//         );
//       }
//       return;
//     }

//     if (frame.vec === Vector.NET_HELLO) {
//       try {
//         this.signal_emit(st, handle, frame);
//       } catch {
//         throw new NetworkFaultError(
//           Errno.ENETUNREACH,
//           "signal hello relay failed",
//         );
//       }

//       let enc_hdr = encoding.b64url_to_bytes(frame.enc_hdr_b64url);
//       let sig = encoding.b64url_to_bytes(frame.sig_b64url) as Signature;
//       this.hello_cb?.(handle, enc_hdr, sig);
//       return;
//     }

//     if (
//       frame.vec === Vector.RTC_SIGNAL_OFFER ||
//       frame.vec === Vector.RTC_SIGNAL_ANSWER ||
//       frame.vec === Vector.RTC_SIGNAL_ICE
//     ) {
//       try {
//         this.signal_emit(st, handle, frame);
//       } catch {
//         throw new NetworkFaultError(Errno.ENETUNREACH, "signal relay failed");
//       }
//       return;
//     }
//   }

//   // ============ Dev/test helpers ============

//   inject_hello_bootstrap(handle: Handle, enc_hdr: Bytes, sig: Signature): void {
//     this.hello_cb?.(handle, enc_hdr, sig);
//   }

//   inject_event(ev: TransportEvent): void {
//     this.emit_event(ev);
//   }

//   inject_link_up(handle: Handle): void {
//     this.emit_event({ vec: Vector.LINK_UP, handle });
//   }

//   inject_link_down(handle: Handle, reason?: string): void {
//     this.emit_event({ vec: Vector.LINK_DOWN, handle, reason });
//   }

//   inject_peer_unstable(handle: Handle, reason?: string): void {
//     this.emit_event({ vec: Vector.PEER_UNSTABLE, handle, reason });
//   }

//   debug_room(handle: Handle): string | null {
//     const st = this.st_by_handle.get(handle);
//     return st?.sig_room ?? null;
//   }
// }
