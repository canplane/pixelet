// @xpute/spec/net/xport/central.ts

import type { Bytes } from "@xpute/spec/lib/kit/array.ts";
import { Errno } from "@xpute/spec/sys/errno.ts";

import type { Signature } from "../crypto.ts";
import type { Dial, Handle, Key } from "./mod.ts";

export type CentralAdvert = {
  // remote endpoint dial hint (transport-facing; CENTRAL policy splits to wire/store URLs)
  dial?: Dial;
};

// ============ Wire/Store Primitives ============

// in-process message (strong)
export type Message = {
  key: Key; // rid 같은 키
  has_blob: 0 | 1; // blob 존재 힌트
  enc_hdr: Bytes;
  sig: Signature;
};

// ============ Central plug contracts (NO optional) ============

// NOTE:
// - *In types are the real socket boundary shapes* (what you receive; untrusted).
// - Message is post-decode (in-process strong type).
// - Always decode_wire_message() on inbound before passing to core logic.

export type Wire = {
  /**
   * Outbound send on an already-established link selected by transport-owned Handle.
   *
   * Contract:
   * - dst is a transport link handle (opaque, transport-owned)
   * - plug routes by handle only (NO overlay PID routing here)
   * - caller is transport (post-routing / post-policy)
   *
   * @throws NetworkFaultError
   */
  send(dst: Handle, msg: Message): void;

  /**
   * Inbound message callback.
   *
   * Contract:
   * - src is the transport link handle for the socket/link instance
   * - payload is already plug-decoded Message (strong type)
   * - transport decides whether unknown handle is accepted (e.g. pre-auth HELLO only)
   */
  on(rx: (src: Handle, msg: Message) => void): void;

  /**
   * Create outbound adjacency link and return transport-visible Handle for that link.
   *
   * Contract:
   * - returned Handle is opaque and transport-facing
   * - Handle SHOULD remain stable across internal reconnect attempts for the same logical link
   *   (plug policy; CENTRAL transport benefits from stable-handle reconnect semantics)
   * - no overlay PID is involved at this boundary
   *
   * @throws NetworkFaultError
   */
  connect(ws_url: Dial): Promise<Handle>;

  /**
   * Link hint callbacks (best-effort, plug-facing physical/session state).
   *
   * Semantics (recommended):
   * - on_handle_up:   initial connect and reconnect edge (handle remains same if stable-handle policy)
   * - on_handle_down: transient disconnect edge (reconnect may still succeed later)
   * - on_handle_dead: terminal failure for this handle/link epoch (reconnect exhausted / path dead)
   *
   * Notes:
   * - Callbacks may be late relative to transport teardown/rebind cleanup.
   * - Transport owns canonical external semantics and may suppress/translate/dedup.
   */
  on_handle_down(cb: (handle: Handle) => void): void;
  on_handle_up(cb: (handle: Handle) => void): void;
  on_handle_dead(cb: (handle: Handle, reason?: string) => void): void;
};

export type Store = {
  /**
   * Store blob on remote/local store path selected by transport-owned Handle + Key.
   *
   * Contract:
   * - dst is opaque transport/store routing key (CENTRAL uses Handle)
   * - Store must not infer overlay identity from dst
   *
   * @throws NetworkFaultError
   */
  put(dst: Handle, key: Key, blob: Bytes): Promise<void>;

  /**
   * Retrieve blob from store path selected by transport-owned Handle + Key.
   *
   * Contract:
   * - ENOENT is normal "not found" outcome (one-shot mailbox miss / expired entry, etc.)
   *
   * @throws NetworkFaultError
   */
  get(dst: Handle, key: Key): Promise<Bytes>;

  /**
   * Bind/unbind plug-local resolved store endpoint for a transport handle.
   *
   * transport-facing generic Store contract:
   * - base_url is a plug-local RESOLVED store base URL (CENTRAL policy result for HTTPPlug)
   * - HTTPPlug interprets it as HTTP base URL
   * - NOT raw advert.dial
   *
   * Notes:
   * - Transport owns dial/advert normalization policy (e.g. split_dial()).
   * - Plug treats base_url as opaque endpoint config for its own runtime.
   */
  set_base_url(handle: Handle, base_url: Dial): void;
  unset_base_url(handle: Handle): void;
};

/**
 * CENTRAL/WS contract (direct node<->node links):
 * - dst routing inside the WS plug is banned (no relay-by-dst).
 * - A Handle identifies the link (transport-owned, opaque) and selects the socket instance/path.
 * - Inbound src_handle is recovered by the plug and reported as the transport Handle
 *   (socket.io socket.id may be used internally but is not the API contract).
 *
 * Therefore payload carries only msg.
 *
 * Event model note (Wire callbacks vs transport events):
 * - Wire handle up/down/dead callbacks are plug-level physical/session hints.
 * - Transport emits canonical overlay-facing TransportEvent(s) and may dedup terminal hints
 *   per PID/live-binding epoch.
 *
 * CACHE note:
 * - CENTRAL may interpret FrameFlag.CACHE as a RID-based split wire/store hint.
 * - WS is the notify/control plane; HTTP/store is the optional fetch plane.
 */
export type WireTx = { msg: Message };
export type WireRx = { msg: Message };
export type WireTxIn = { msg: unknown };
export type WireRxIn = { msg: unknown };

/**
 * xport errno mapping contract (HTTP / WS / guard)
 *
 * Scope:
 * - Standardizes errno mapping for transport-adjacent plugs:
 *   - HTTP store plug (client/server remote fetch path)
 *   - WS wire plug (connect/send/decode path)
 *   - guard/decode validators at xport boundary
 *
 * Goals:
 * - Stable observability across runtimes (web/node/server)
 * - Consistent retry / drop / alert policy by errno
 * - Avoid plug-specific ad-hoc status/error translations
 *
 * Error class policy (recommended):
 * - InvariantError:
 *   - local caller misuse / contract violation / missing local binding
 *   - examples: invalid handle/key, empty blob, missing base_url
 *
 * - NetworkFaultError:
 *   - remote peer/path/protocol/network failures
 *   - examples: fetch/connect timeout, remote HTTP status, socket disconnected
 *
 * Guard / decode mapping:
 * - invalid handle/key/path param format        -> EINVAL
 * - empty blob (when payload is required)       -> EINVAL
 * - oversized blob                              -> EFBIG
 * - malformed wire/message/frame payload        -> EBADMSG (or drop silently with telemetry)
 *
 * HTTP status -> errno (remote fetch path):
 * - 400 -> EBADMSG      (protocol/request shape invalid)
 * - 401 -> EACCES
 * - 403 -> EACCES
 * - 404 -> ENOENT
 * - 408 -> ETIMEDOUT
 * - 409 -> EEXIST       (conflict/policy collision)
 * - 413 -> EFBIG
 * - 429 -> EAGAIN       (retry later / rate-limited)
 * - 5xx -> EHOSTUNREACH (remote endpoint/server-side path unavailable/failing)
 * - else -> EIO
 *
 * HTTP client runtime failures (fetch throws):
 * - DNS/connect/reset/CORS/network failure -> ENETUNREACH
 *
 * WS plug mapping (best-effort):
 * - unknown handle / no route               -> ENOENT
 * - socket exists but not connected         -> ENOTCONN
 * - connect refused / handshake rejected    -> ECONNREFUSED | EACCES (best-effort parse)
 * - timeout while connecting                -> ETIMEDOUT (best-effort parse)
 * - emit/send internal failure              -> EIO
 * - invalid inbound wire payload            -> EBADMSG (usually drop + telemetry)
 *
 * Notes:
 * - Mapping is intentionally lossy and operationally oriented.
 * - Prefer stable errno semantics over preserving every framework-specific detail.
 */
export function errno_from_http_status(status: number): Errno {
  switch (status) {
    case 400:
      return Errno.EBADMSG;
    case 401:
    case 403:
      return Errno.EACCES;
    case 404:
    case 410:
      return Errno.ENOENT;
    case 408:
      return Errno.ETIMEDOUT;
    case 409:
      return Errno.EEXIST;
    case 422:
      return Errno.EBADMSG;
    case 413:
      return Errno.EFBIG;
    case 429:
      return Errno.EAGAIN;
    case 501:
      return Errno.ENOTSUP;
    case 503:
      return Errno.EAGAIN;
    case 504:
      return Errno.ETIMEDOUT;
  }
  if (status >= 500) return Errno.EHOSTUNREACH;
  return Errno.EIO;
}
export function errno_from_ws_connect_error(err: unknown): Errno {
  let msg = "";
  let desc: unknown = null;
  let code: unknown = null;

  if (err && typeof err === "object") {
    const o = err as { message?: unknown; description?: unknown; code?: unknown };
    msg = typeof o.message === "string" ? o.message : "";
    desc = o.description;
    code = o.code;
  }

  if (typeof code === "number") {
    if (code === 401 || code === 403) return Errno.EACCES;
    if (code === 408) return Errno.ETIMEDOUT;
    if (code === 429) return Errno.EAGAIN;
    if (code === 500 || code === 502 || code === 503 || code === 504) return Errno.EHOSTUNREACH;
  }
  if (typeof desc === "number") {
    if (desc === 401 || desc === 403) return Errno.EACCES;
    if (desc === 408) return Errno.ETIMEDOUT;
    if (desc === 429) return Errno.EAGAIN;
    if (desc === 500 || desc === 502 || desc === 503 || desc === 504) return Errno.EHOSTUNREACH;
  }

  let s = `${msg} ${typeof desc === "string" ? desc : ""} ${typeof code === "string" ? code : ""}`.toLowerCase();

  // Node/browser/network-ish codes (socket.io connect_error can surface these)
  if (s.includes("etimedout") || s.includes("timeout")) return Errno.ETIMEDOUT;
  if (s.includes("econnrefused")) return Errno.ECONNREFUSED;
  if (s.includes("econnreset")) return Errno.ECONNRESET;
  if (s.includes("enotfound") || s.includes("eai_again")) return Errno.ENETUNREACH;
  if (s.includes("ehostunreach")) return Errno.EHOSTUNREACH;
  if (s.includes("enetunreach")) return Errno.ENETUNREACH;
  if (s.includes("unauthorized") || s.includes("forbidden")) return Errno.EACCES;
  if (s.includes("websocket error") || s.includes("xhr poll error") || s.includes("transport error")) {
    return Errno.ENETUNREACH;
  }

  return Errno.ECONNREFUSED;
}

/**
 * CENTRAL dial split (CENTRAL policy only)
 *
 * Dial semantics:
 * - Dial is a bootstrap hint consumed by the transport.
 * - In CENTRAL, Dial is treated as URL-like and converted between wire/store endpoints.
 *
 * Policy:
 * - http(s) Dial  -> ws(s) wire URL + http(s) store base URL
 * - ws(s) Dial    -> ws(s) wire URL + http(s) store base URL (derived)
 *
 * Boundary:
 * - advert/manifest/transport contracts carry Dial hint (`dial`)
 * - Store plug receives resolved `base_url` (HTTPPlug interprets as HTTP base URL)
 *
 * Note:
 * - This URL handling is CENTRAL-specific policy.
 * - Dial itself is not globally constrained to URL/URI in the Transport ABI.
 */
export function split_dial(dial: Dial): { ws_url: string; http_base_url: string } {
  const s = String(dial);

  if (s.startsWith("https://")) {
    return { ws_url: `wss://${s.slice("https://".length)}`, http_base_url: s };
  }
  if (s.startsWith("http://")) {
    return { ws_url: `ws://${s.slice("http://".length)}`, http_base_url: s };
  }
  if (s.startsWith("wss://")) {
    return { ws_url: s, http_base_url: `https://${s.slice("wss://".length)}` };
  }
  if (s.startsWith("ws://")) {
    return { ws_url: s, http_base_url: `http://${s.slice("ws://".length)}` };
  }

  // dial_prefixes() allowlist should prevent reaching here; keep loud failure.
  throw new Error(`split_dial: unsupported dial: ${s}`);
}
