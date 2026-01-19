// @pxl/web/xpute/plug/http.ts - Browser

import type { Bytes } from "@xpute/spec/lib/kit/array.ts";
import { Errno } from "@xpute/spec/sys/errno.ts";
import { InvariantError, NetworkFaultError, XputeError } from "@xpute/spec/sys/error.ts";

import type { Dial, Handle, Key } from "@xpute/spec/net/xport/mod.ts";
import { errno_from_http_status, type Store } from "@xpute/spec/net/xport/central.ts";
import { decode_handle, decode_key, ok_blob_len } from "@xpute/core/net/kit/guard.ts";

const esc = (s: string): string => encodeURIComponent(s);
const trim_trailing_slash = (s: string): string => s.replace(/\/+$/, "");

export class HTTPPlug implements Store {
  // plug-local resolved base URL by handle (HTTPPlug treats this as HTTP base URL; not raw advert.dial)
  private readonly base_url_by_handle = new Map<Handle, Dial>();

  attach(): void {}

  detach(): void {
    this.base_url_by_handle.clear();
  }

  /**
   * Handle semantics:
   * - handle is a transport-local link identifier (NOT overlay identity).
   * - for CENTRAL HTTP store, handle selects the resolved base_url bucket.
   *
   * Contract:
   * - base_url is plug-local resolved HTTP base URL (CENTRAL policy result),
   *   and HTTPPlug interprets it as HTTP base URL.
   *   not raw advert.dial.
   */
  set_base_url(handle: Handle, base_url: Dial): void {
    this.base_url_by_handle.set(handle, trim_trailing_slash(base_url));
  }

  unset_base_url(handle: Handle): void {
    this.base_url_by_handle.delete(handle);
  }

  private base_url_of(dst: Handle): Dial | null {
    const url = this.base_url_by_handle.get(dst);
    return url ?? null;
  }

  /** @throws NetworkFaultError */
  async put(dst: Handle, key: Key, blob: Bytes): Promise<void> {
    const dst2 = decode_handle(dst, "HTTPPlug[client].put"); // opaque handle
    const key2 = decode_key(key, "HTTPPlug[client].put");
    if (!dst2 || !key2) throw new InvariantError(Errno.EINVAL, "invalid args");
    if (blob.byteLength <= 0) {
      ok_blob_len(blob.byteLength, "HTTPPlug[client].put"); // keep drop telemetry
      throw new InvariantError(Errno.EINVAL, "empty blob");
    }
    if (!ok_blob_len(blob.byteLength, "HTTPPlug[client].put")) {
      throw new NetworkFaultError(Errno.EFBIG, "blob too big");
    }

    const base_url = this.base_url_of(dst2);
    if (!base_url) throw new InvariantError(Errno.ENOENT, "missing base_url");
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

  /** @throws NetworkFaultError */
  async get(dst: Handle, key: Key): Promise<Bytes> {
    const dst2 = decode_handle(dst, "HTTPPlug[client].get"); // opaque handle
    const key2 = decode_key(key, "HTTPPlug[client].get");
    if (!dst2 || !key2) throw new InvariantError(Errno.EINVAL, "invalid args");

    const base_url = this.base_url_of(dst2);
    if (!base_url) throw new InvariantError(Errno.ENOENT, "missing base_url");

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
}
