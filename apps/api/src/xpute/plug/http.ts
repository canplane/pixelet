// @pxl/api/xpute/plug/http.ts - Server

import type { Hono } from "@hono/hono";

import { as_bytes, type Bytes } from "@xpute/spec/lib/kit/array.ts";
import { Errno } from "@xpute/spec/sys/errno.ts";
import { NetworkFaultError, XputeError } from "@xpute/spec/sys/error.ts";

import type { Dial, Handle, Key } from "@xpute/spec/net/xport/mod.ts";
import { errno_from_http_status, type Store } from "@xpute/spec/net/xport/central.ts";
import { BOX_TTL_MS, decode_handle, decode_key, MAX_BOX_ENTRIES, ok_blob_len } from "@xpute/core/net/kit/guard.ts";

const esc = (s: string): string => encodeURIComponent(s);
const trim_trailing_slash = (s: string): string => s.replace(/\/+$/, "");

type Entry = { blob: Bytes; ts: number };
const SWEEP_INTERVAL_MS = 60_000;

// HTTP status codes (replaces @nestjs/common HttpStatus)
const HTTP = {
  OK: 200,
  ACCEPTED: 202,
  BAD_REQUEST: 400,
  NOT_FOUND: 404,
  UNSUPPORTED_MEDIA_TYPE: 415,
  PAYLOAD_TOO_LARGE: 413,
  INSUFFICIENT_STORAGE: 507,
} as const;

export class HTTPPlug implements Store {
  attach(router: Hono): void {
    // idempotent-ish: avoid double-register if you care; else assume called once
    this.register(router);
  }

  detach(): void {}

  /**
   * Store routing key contract:
   * - dst is transport-owned opaque key.
   * - CENTRAL uses Handle as dst.
   * - Other transports may use Handle or PID by policy.
   * - Store must not parse/interpret dst.
   */

  private readonly box = new Map<string, Entry>();
  private last_sweep_ms = 0;

  // handle-scoped remote base URL binding (plug-local resolved HTTP base URL).
  // source may be seed split_dial() result or HELLO advert.dial normalized by CENTRAL transport.
  private readonly base_url_by_handle = new Map<Handle, Dial>();

  // bind resolved base URL for a specific adjacency handle.
  // HTTPPlug interprets base_url as HTTP base URL.
  // (NOT raw advert.dial; CENTRAL transport normalizes dial -> http_base_url)
  set_base_url(handle: Handle, base_url: Dial): void {
    this.base_url_by_handle.set(handle, trim_trailing_slash(base_url));
  }

  // handle_down cleanup (best-effort)
  unset_base_url(handle: Handle): void {
    this.base_url_by_handle.delete(handle);
  }

  private base_url_of(dst: Handle): Dial | null {
    return this.base_url_by_handle.get(dst) ?? null;
  }

  private maybe_sweep(): void {
    if (BOX_TTL_MS <= 0) return;
    const now = Date.now();
    if (now - this.last_sweep_ms < SWEEP_INTERVAL_MS) return;
    this.last_sweep_ms = now;
    this.sweep_stale();
  }

  private static key_of(dst: Handle, key: Key): string {
    return `${dst}/${key}`;
  }

  private local_put(dst: Handle, key: Key, blob: Bytes): 202 | 507 {
    this.maybe_sweep();
    const k = HTTPPlug.key_of(dst, key);
    if (!this.box.has(k) && this.box.size >= MAX_BOX_ENTRIES) return HTTP.INSUFFICIENT_STORAGE;
    this.box.set(k, { blob, ts: Date.now() });
    return HTTP.ACCEPTED;
  }

  private local_get(dst: Handle, key: Key): { status: 200; blob: Bytes } | { status: 404 } {
    this.maybe_sweep();
    const k = HTTPPlug.key_of(dst, key);
    const e = this.box.get(k);
    if (!e) return { status: HTTP.NOT_FOUND };
    if (BOX_TTL_MS > 0 && Date.now() - e.ts > BOX_TTL_MS) {
      this.box.delete(k);
      return { status: HTTP.NOT_FOUND };
    }
    this.box.delete(k); // one-shot
    return { status: HTTP.OK, blob: e.blob };
  }

  /** @throws NetworkFaultError */
  async put(dst: Handle, key: Key, blob: Bytes): Promise<void> {
    const dst2 = decode_handle(dst, "HTTPPlug[server].put");
    const key2 = decode_key(key, "HTTPPlug[server].put");
    if (!dst2 || !key2) throw new NetworkFaultError(Errno.EINVAL, "invalid args");
    if (blob.byteLength <= 0) {
      ok_blob_len(blob.byteLength, "HTTPPlug[server].put");
      throw new NetworkFaultError(Errno.EINVAL, "empty blob");
    }
    if (!ok_blob_len(blob.byteLength, "HTTPPlug[server].put")) throw new NetworkFaultError(Errno.EFBIG, "blob too big");

    // If this handle is bound to a remote HTTP store, push there (server<->server, etc).
    const base_url = this.base_url_of(dst2);
    if (base_url) {
      try {
        const res = await fetch(`${base_url}/x/${esc(dst2)}/${esc(key2)}`, {
          method: "POST",
          headers: { "Content-Type": "application/octet-stream" },
          body: blob,
        });
        if (res.ok) return;
        throw new NetworkFaultError(errno_from_http_status(res.status), `${res.status}`);
      } catch (err) {
        if (err instanceof XputeError) throw err;
        throw new NetworkFaultError(Errno.ENETUNREACH, "fetch failed", { cause: err });
      }
    }

    // Otherwise, keep locally (browser peers can pull from this server's /x).
    const status = this.local_put(dst2, key2, blob);
    if (status === HTTP.ACCEPTED) return;
    if (status === HTTP.INSUFFICIENT_STORAGE) throw new NetworkFaultError(Errno.ENOSPC, "box full");
    throw new NetworkFaultError(Errno.EIO, `local_put:${status}`);
  }

  /** @throws NetworkFaultError */
  async get(dst: Handle, key: Key): Promise<Bytes> {
    const dst2 = decode_handle(dst, "HTTPPlug[server].get");
    const key2 = decode_key(key, "HTTPPlug[server].get");
    if (!dst2 || !key2) throw new NetworkFaultError(Errno.EINVAL, "invalid args");

    // If bound to a remote HTTP store, pull from there.
    const base_url = this.base_url_of(dst2);
    if (base_url) {
      try {
        const res = await fetch(`${base_url}/x/${esc(dst2)}/${esc(key2)}`, { method: "GET" });
        if (!res.ok) throw new NetworkFaultError(errno_from_http_status(res.status), `${res.status}`);
        const buf = await res.arrayBuffer();
        return new Uint8Array(buf);
      } catch (err) {
        if (err instanceof XputeError) throw err;
        throw new NetworkFaultError(Errno.ENETUNREACH, "fetch failed", { cause: err });
      }
    }

    let r = this.local_get(dst2, key2);
    if (r.status === HTTP.OK) return r.blob;
    if (r.status === HTTP.NOT_FOUND) throw new NetworkFaultError(Errno.ENOENT, "not found");
    throw new NetworkFaultError(Errno.EIO, `local_get:${JSON.stringify(r)}`);
  }

  private sweep_stale(): void {
    if (BOX_TTL_MS <= 0) return;
    const now = Date.now();
    for (const [k, e] of this.box) {
      if (now - e.ts > BOX_TTL_MS) this.box.delete(k);
    }
  }

  /** Register /x routes on a Hono app */
  register(router: Hono): void {
    router.post("/x/:dst/:key", async (c) => {
      const ct = c.req.header("content-type") ?? "";
      if (ct !== "application/octet-stream") return c.body(null, HTTP.UNSUPPORTED_MEDIA_TYPE);

      const dst = c.req.param("dst");
      const key = c.req.param("key");
      const dst2 = decode_handle(dst, "HTTPPlug[server].put");
      const key2 = decode_key(key, "HTTPPlug[server].put");
      if (!dst2 || !key2) return c.body(null, HTTP.BAD_REQUEST);

      const buf = await c.req.arrayBuffer();
      const blob = as_bytes(buf);
      if (blob.byteLength <= 0) {
        ok_blob_len(blob.byteLength, "HTTPPlug[server].put");
        return c.body(null, HTTP.BAD_REQUEST);
      }
      if (!ok_blob_len(blob.byteLength, "HTTPPlug[server].put")) {
        return c.body(null, HTTP.PAYLOAD_TOO_LARGE);
      }

      const status = this.local_put(dst2, key2, blob);
      return c.body(null, status);
    });

    router.get("/x/:dst/:key", (c) => {
      const dst = c.req.param("dst");
      const key = c.req.param("key");
      const dst2 = decode_handle(dst, "HTTPPlug[server].get");
      const key2 = decode_key(key, "HTTPPlug[server].get");
      if (!dst2 || !key2) return c.body(null, HTTP.BAD_REQUEST);

      // controller path is local store endpoint; must not reuse remote-forwarding path
      const r = this.local_get(dst2, key2);
      if (r.status !== HTTP.OK) return c.body(null, r.status);
      return new Response(r.blob, {
        headers: { "Content-Type": "application/octet-stream" },
      });
    });
  }
}
