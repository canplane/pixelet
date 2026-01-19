// // @pxl/api-nest/xpute/plug/http.ts - Server

// import {
//   Controller,
//   Get,
//   HttpStatus,
//   OnApplicationBootstrap,
//   OnApplicationShutdown,
//   Param,
//   Post,
//   Req,
//   Res,
// } from "@nestjs/common";
// import type { FastifyReply, FastifyRequest } from "fastify";

// import { as_bytes, type Bytes } from "@xpute/spec/lib/kit/array.ts";
// import { Errno } from "@xpute/spec/sys/errno.ts";
// import { NetworkFaultError, XputeError } from "@xpute/spec/sys/error.ts";

// import type { Dial, Handle, Key } from "@xpute/spec/net/xport/mod.ts";
// import {
//   errno_from_http_status,
//   type Store,
// } from "@xpute/spec/net/central.ts";
// import {
//   BOX_TTL_MS,
//   decode_handle,
//   decode_key,
//   MAX_BOX_ENTRIES,
//   ok_blob_len,
// } from "@xpute/core/net/kit/guard.ts";

// const esc = (s: string): string => encodeURIComponent(s);
// const trim_trailing_slash = (s: string): string => s.replace(/\/+$/, "");

// type Entry = { blob: Bytes; ts: number };
// const SWEEP_INTERVAL_MS = 60_000;

// @Controller("x") // Nest entrypoint, semantic role = Store plug
// export class HTTPPlug
//   implements Store, OnApplicationBootstrap, OnApplicationShutdown {
//   private sweep_timer: any = null;

//   onApplicationBootstrap() {
//     this.sweep_timer = setInterval(() => this.sweep_stale(), SWEEP_INTERVAL_MS);
//     this.sweep_timer?.unref?.();
//   }

//   onApplicationShutdown() {
//     if (!this.sweep_timer) return;
//     clearInterval(this.sweep_timer);
//     this.sweep_timer = null;
//   }

//   /**
//    * Store routing key contract:
//    * - dst is transport-owned opaque key.
//    * - CENTRAL uses Handle as dst.
//    * - Other transports may use Handle or PID by policy.
//    * - Store must not parse/interpret dst.
//    */

//   private readonly box = new Map<string, Entry>();
//   private last_sweep_ms = 0;

//   // handle-scoped remote base URL binding (plug-local resolved HTTP base URL).
//   // source may be seed split_dial() result or HELLO advert.dial normalized by CENTRAL transport.
//   private readonly base_url_by_handle = new Map<Handle, Dial>();

//   // bind resolved base URL for a specific adjacency handle.
//   // HTTPPlug interprets base_url as HTTP base URL.
//   // (NOT raw advert.dial; CENTRAL transport normalizes dial -> http_base_url)
//   set_base_url(handle: Handle, base_url: Dial): void {
//     this.base_url_by_handle.set(handle, trim_trailing_slash(base_url));
//   }

//   // handle_down cleanup (best-effort)
//   unset_base_url(handle: Handle): void {
//     this.base_url_by_handle.delete(handle);
//   }

//   private base_url_of(dst: Handle): Dial | null {
//     return this.base_url_by_handle.get(dst) ?? null;
//   }

//   private maybe_sweep(): void {
//     if (BOX_TTL_MS <= 0) return;
//     const now = Date.now();
//     if (now - this.last_sweep_ms < SWEEP_INTERVAL_MS) return;
//     this.last_sweep_ms = now;
//     this.sweep_stale();
//   }

//   private static key_of(dst: Handle, key: Key): string {
//     return `${dst}/${key}`;
//   }

//   private local_put(dst: Handle, key: Key, blob: Bytes): HttpStatus {
//     this.maybe_sweep();
//     const k = HTTPPlug.key_of(dst, key);
//     if (!this.box.has(k) && this.box.size >= MAX_BOX_ENTRIES) {
//       return HttpStatus.INSUFFICIENT_STORAGE;
//     }
//     this.box.set(k, { blob, ts: Date.now() });
//     return HttpStatus.ACCEPTED;
//   }

//   private local_get(
//     dst: Handle,
//     key: Key,
//   ): { status: HttpStatus.OK; blob: Bytes } | { status: HttpStatus.NOT_FOUND } {
//     this.maybe_sweep();
//     const k = HTTPPlug.key_of(dst, key);
//     const e = this.box.get(k);
//     if (!e) return { status: HttpStatus.NOT_FOUND };
//     if (BOX_TTL_MS > 0 && Date.now() - e.ts > BOX_TTL_MS) {
//       this.box.delete(k);
//       return { status: HttpStatus.NOT_FOUND };
//     }
//     this.box.delete(k); // one-shot
//     return { status: HttpStatus.OK, blob: e.blob };
//   }

//   /** @throws NetworkFaultError */
//   async put(dst: Handle, key: Key, blob: Bytes): Promise<void> {
//     const dst2 = decode_handle(dst, "HTTPPlug[server].put");
//     const key2 = decode_key(key, "HTTPPlug[server].put");
//     if (!dst2 || !key2) {
//       throw new NetworkFaultError(Errno.EINVAL, "invalid args");
//     }
//     if (blob.byteLength <= 0) {
//       ok_blob_len(blob.byteLength, "HTTPPlug[server].put");
//       throw new NetworkFaultError(Errno.EINVAL, "empty blob");
//     }
//     if (!ok_blob_len(blob.byteLength, "HTTPPlug[server].put")) {
//       throw new NetworkFaultError(Errno.EFBIG, "blob too big");
//     }

//     // If this handle is bound to a remote HTTP store, push there (server<->server, etc).
//     const base_url = this.base_url_of(dst2);
//     if (base_url) {
//       try {
//         const res = await fetch(`${base_url}/x/${esc(dst2)}/${esc(key2)}`, {
//           method: "POST",
//           headers: { "Content-Type": "application/octet-stream" },
//           body: blob,
//         });
//         if (res.ok) return;
//         throw new NetworkFaultError(
//           errno_from_http_status(res.status),
//           `${res.status}`,
//         );
//       } catch (err) {
//         if (err instanceof XputeError) throw err;
//         throw new NetworkFaultError(Errno.ENETUNREACH, "fetch failed", {
//           cause: err,
//         });
//       }
//     }

//     // Otherwise, keep locally (browser peers can pull from this server’s /x).
//     let status = this.local_put(dst2, key2, blob);
//     if (status === HttpStatus.ACCEPTED) return;
//     if (status === HttpStatus.INSUFFICIENT_STORAGE) {
//       throw new NetworkFaultError(Errno.ENOSPC, "box full");
//     }
//     throw new NetworkFaultError(Errno.EIO, `local_put:${status}`);
//   }

//   /** @throws NetworkFaultError */
//   async get(dst: Handle, key: Key): Promise<Bytes> {
//     const dst2 = decode_handle(dst, "HTTPPlug[server].get");
//     const key2 = decode_key(key, "HTTPPlug[server].get");
//     if (!dst2 || !key2) {
//       throw new NetworkFaultError(Errno.EINVAL, "invalid args");
//     }

//     // If bound to a remote HTTP store, pull from there.
//     const base_url = this.base_url_of(dst2);
//     if (base_url) {
//       try {
//         const res = await fetch(`${base_url}/x/${esc(dst2)}/${esc(key2)}`, {
//           method: "GET",
//         });
//         if (!res.ok) {
//           throw new NetworkFaultError(
//             errno_from_http_status(res.status),
//             `${res.status}`,
//           );
//         }
//         const buf = await res.arrayBuffer();
//         return new Uint8Array(buf);
//       } catch (err) {
//         if (err instanceof XputeError) throw err;
//         throw new NetworkFaultError(Errno.ENETUNREACH, "fetch failed", {
//           cause: err,
//         });
//       }
//     }

//     let r = this.local_get(dst2, key2);
//     if (r.status === HttpStatus.OK) return r.blob;
//     if (r.status === HttpStatus.NOT_FOUND) {
//       throw new NetworkFaultError(Errno.ENOENT, "not found");
//     }
//     throw new NetworkFaultError(Errno.EIO, `local_get:${JSON.stringify(r)}`);
//   }

//   private sweep_stale(): void {
//     if (BOX_TTL_MS <= 0) return;
//     const now = Date.now();
//     for (const [k, e] of this.box) {
//       if (now - e.ts > BOX_TTL_MS) this.box.delete(k);
//     }
//   }

//   @Post(":dst/:key")
//   async http_put(
//     @Param("dst") dst: string,
//     @Param("key") key: string,
//     @Req() req: FastifyRequest,
//     @Res() res: FastifyReply,
//   ) {
//     const dst2 = decode_handle(dst, "HTTPPlug[server].put"); // opaque handle
//     const key2 = decode_key(key, "HTTPPlug[server].put");
//     if (!dst2 || !key2) return res.status(HttpStatus.BAD_REQUEST).send();

//     const blob = as_bytes(req.body);
//     if (blob.byteLength <= 0) {
//       ok_blob_len(blob.byteLength, "HTTPPlug[server].put"); // keep drop telemetry
//       return res.status(HttpStatus.BAD_REQUEST).send();
//     }
//     if (!ok_blob_len(blob.byteLength, "HTTPPlug[server].put")) {
//       return res.status(HttpStatus.PAYLOAD_TOO_LARGE).send();
//     }

//     let status = this.local_put(dst2, key2, blob);
//     return res.status(status).send();
//   }

//   @Get(":dst/:key")
//   async http_get(
//     @Param("dst") dst: string,
//     @Param("key") key: string,
//     @Res() res: FastifyReply,
//   ) {
//     const dst2 = decode_handle(dst, "HTTPPlug[server].get"); // opaque handle
//     const key2 = decode_key(key, "HTTPPlug[server].get");
//     if (!dst2 || !key2) return res.status(HttpStatus.BAD_REQUEST).send();

//     // controller path is local store endpoint; must not reuse remote-forwarding path
//     let r = this.local_get(dst2, key2);
//     if (r.status !== HttpStatus.OK) return res.status(r.status).send();
//     res.header("Content-Type", "application/octet-stream");
//     return res.send(r.blob);
//   }
// }
