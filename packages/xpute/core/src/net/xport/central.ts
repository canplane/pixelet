// @xpute/core/net/xport/central.ts

import type { u32 } from "@xpute/spec/lib/kit/word.ts";
import type { Bytes } from "@xpute/spec/lib/kit/array.ts";
import { Errno } from "@xpute/spec/sys/errno.ts";
import { InvariantError, NetworkFaultError, XputeError } from "@xpute/spec/sys/error.ts";

import type { PID, Signature } from "@xpute/spec/net/crypto.ts";
import type { RID } from "@xpute/spec/rpc/rid.ts";
import { Vector } from "@xpute/spec/control/vec.ts";
import type { CentralTransport as _CentralTransport, Dial, Handle, Peer, RxFn, TransportEventFn } from "@xpute/spec/net/xport/mod.ts";
import { TransportKind } from "@xpute/spec/net/xport/mod.ts";
import type { CentralAdvert, Message, Store, Wire } from "@xpute/spec/net/xport/central.ts";
import { split_dial } from "@xpute/spec/net/xport/central.ts";

import { sniff_wire_vec } from "@xpute/spec/rpc/wire_header.ts";

import { drop as _drop } from "../kit/drop.ts";
const drop = (why: string, errno?: Errno): void => _drop("central", { why, errno });

export class CentralTransport implements _CentralTransport {
  readonly kind = TransportKind.CENTRAL;

  // CACHE interpretation (CENTRAL policy):
  // - CACHE is not a transport selector by itself.
  // - WS remains the signaling/control plane.
  // - HTTP/store is the optional RID-addressable fetch plane.
  // - For body-bearing sends, CENTRAL may stage payload into store/outbox and
  //   use wire only as notification.
  // - For bodyless requests, CACHE may indicate deterministic RID lookup
  //   semantics, but the request frame itself still flows over wire.

  // TODO(security/ops):
  // - per-handle wire send rate cap / burst cap (especially pre-auth HELLO path)
  // - store put/get timeout policy and retry policy documentation (avoid hidden indefinite stalls)
  // - degraded-path counters: wire_send_fail, store_put_fail, store_get_fail, unreachable_emit
  // - event storm suppression/coalescing for repeated PEER_UNSTABLE on same handle
  // - dead_grace tuning by environment (dev/test/prod) and explicit rationale doc
  // - dial allowlist / scheme validation at transport boundary (defense-in-depth)
  // - stale-handle lifecycle audit (rebind races, timer cleanup, store bucket cleanup)
  // - chaos tests: wire flap, store flap, split-brain handle rebind, delayed PUT then rebind
  // - payload size observability (enc_hdr/blob distribution) to validate MAX_BODY assumptions upstream

  private rx: RxFn | null = null;
  private ev: TransportEventFn | null = null;
  // routing: pid <-> handle (transport-owned)
  private readonly handle_by_pid: Map<PID, Handle> = new Map();
  private readonly pid_by_handle: Map<Handle, PID> = new Map();
  // pid-scoped terminal latch for CURRENT live binding epoch only.
  // Set when CENTRAL emits PEER_UNREACHABLE for a pid with a live handle.
  // Cleared on bind_handle_pid() when a new/renewed authenticated binding is established.
  private readonly unreachable_pid: Set<PID> = new Set();
  private readonly store_down_by_handle: Set<Handle> = new Set();
  private readonly wire_down_by_handle: Set<Handle> = new Set();
  private readonly down_since_ms_by_handle: Map<Handle, number> = new Map();

  private dead_timer: any = null;

  // continuous degraded-time policy:
  // - start counting on first degradation edge (wire_down OR store_down)
  // - clear only when ALL internal lanes recover
  // - if degraded state persists for grace => PEER_UNREACHABLE
  private readonly dead_grace_ms: u32;

  /**
   * CENTRAL dial policy (transport-owned):
   * - parse Dial via split_dial()
   * - create adjacency wire link (ws/wss)
   * - configure Store base_url bucket (http/https) for the allocated Handle
   *
   * IMPORTANT:
   * - Handle must stay opaque (NO PID, NO Dial embedded).
   * - One remote node == one (ws,http) set (1 PID <-> 1 Handle <-> 1 WS + 1 HTTP).
   */

  constructor(
    private readonly wire: Wire,
    private readonly store: Store,
    dead_grace_ms: u32 = 10_000, // dead_grace_ms (policy)
  ) {
    this.dead_grace_ms = dead_grace_ms;

    wire.on((src, msg) => void this.on_wire(src, msg));

    this.wire.on_handle_up((handle: Handle) => {
      const pid = this.pid_by_handle.get(handle);
      this.wire_down_by_handle.delete(handle);
      // NOTE: store lane is independent of wire lane; do NOT clear store binding here.
      // NOTE: keep store_down marker HTTP-observation-based by default (no implicit clear on wire-up).
      // (If policy changes, an optional clear could be added here.)
      // this.store_down_by_handle.delete(handle);
      // continuous degraded-time policy:
      // clear down_since only if ALL internal lanes are recovered.
      if (!this.wire_down_by_handle.has(handle) && !this.store_down_by_handle.has(handle)) {
        this.down_since_ms_by_handle.delete(handle);
      }
      // hint event (connect/reconnect edge)
      this.ev?.({ vec: Vector.LINK_UP, handle, pid });
    });

    this.wire.on_handle_down((handle: Handle) => {
      const pid = this.pid_by_handle.get(handle);
      const was_down = this.wire_down_by_handle.has(handle);
      this.wire_down_by_handle.add(handle);
      // IMPORTANT (stable-handle policy):
      // wire down/up may happen across reconnects while the handle remains the same.
      // Do NOT drop handle-scoped store binding here, otherwise post-reconnect store ops break.
      if (!this.down_since_ms_by_handle.has(handle)) this.down_since_ms_by_handle.set(handle, Date.now());
      this.ensure_dead_timer();
      if (!was_down) {
        this.ev?.({ vec: Vector.LINK_DOWN, handle, pid, reason: "disconnect" });
        this.ev?.({ vec: Vector.PEER_UNSTABLE, handle, pid, reason: "link_down" });
      }
    });

    // wire plug terminal failure (reconnect exhausted) -> PEER_UNREACHABLE (pid-scoped)
    this.wire.on_handle_dead((handle: Handle, reason?: string) => {
      const pid = this.pid_by_handle.get(handle);
      if (!pid) return;
      this.emit_unreachable_terminal(pid, reason ?? "wire_dead");
    });
  }

  is_pre_auth_allowed(enc_hdr: Bytes, has_body: 0 | 1): boolean {
    if (has_body) return false;
    const vec = sniff_wire_vec(enc_hdr);
    return vec === Vector.NET_HELLO;
  }

  dial_prefixes(): readonly string[] {
    return ["http://", "https://", "ws://", "wss://"];
  }

  async dial_seed(dial: Dial): Promise<Handle> {
    const { ws_url, http_base_url } = split_dial(dial);

    const handle = await this.wire.connect(ws_url);
    this.store.set_base_url(handle, http_base_url); // seed-side bucket (HTTPPlug interprets as base URL)
    return handle;
  }

  bind_handle_pid(handle: Handle, pid: PID): void {
    const prev_pid = this.pid_by_handle.get(handle);
    if (prev_pid && prev_pid !== pid) throw new InvariantError(Errno.EEXIST, "handle->pid conflict");
    const prev_handle = this.handle_by_pid.get(pid);
    if (prev_pid === pid && prev_handle === handle) {
      // duplicate bind (HELLO retry/dup OK): refresh live epoch latch
      this.unreachable_pid.delete(pid);
      return;
    }
    if (prev_handle && prev_handle !== handle && this.pid_by_handle.get(prev_handle) === pid) {
      // allowed rebind path; old handle cleanup below
    }
    const old = this.handle_by_pid.get(pid);
    if (old && old !== handle) {
      // handle rebinding (reconnect / multipath evolution):
      // old handle becomes stale; ensure no dead-sweep targets it and no reverse mapping remains.
      this.pid_by_handle.delete(old);
      this.wire_down_by_handle.delete(old);
      this.store_down_by_handle.delete(old);
      this.down_since_ms_by_handle.delete(old);
      this.store.unset_base_url(old);
    }

    this.pid_by_handle.set(handle, pid);
    this.handle_by_pid.set(pid, handle);
    this.unreachable_pid.delete(pid); // new live binding epoch starts
  }

  private emit_unreachable_terminal(pid: PID, reason: string, errno?: Errno): void {
    // pid-scoped terminal hint de-dup for current live binding epoch only.
    const handle = this.handle_by_pid.get(pid);
    if (!handle) return; // stale/late terminal hint

    if (this.unreachable_pid.has(pid)) return;
    this.unreachable_pid.add(pid);

    // terminal decision => drop binding; force re-HELLO on future reconnect
    this.drop_handle(handle);
    this.ev?.({ vec: Vector.PEER_UNREACHABLE, pid, reason, errno });
  }

  bind_handle_advert(handle: Handle, advert?: CentralAdvert): void {
    // stale/unknown handle guard:
    // advert application may race with teardown/rebind cleanup paths.
    // NOTE:
    // - This assumes CENTRAL advert bind is applied after HELLO/bind (post-auth path).
    // - If pre-auth advert binding is intended, remove this guard or track seed handles explicitly.
    if (
      !this.pid_by_handle.has(handle) &&
      !this.wire_down_by_handle.has(handle) &&
      !this.store_down_by_handle.has(handle) &&
      !this.down_since_ms_by_handle.has(handle)
    ) {
      return;
    }

    const dial = advert?.dial;
    if (!dial) {
      this.store.unset_base_url(handle);
      return;
    }
    // advert carries Dial hint (raw / transport-facing). Store plug expects resolved HTTP base URL.
    // CENTRAL policy normalization lives here (not in the generic Store plug contract).
    let http_base_url: string;
    try {
      ({ http_base_url } = split_dial(dial));
    } catch {
      // malformed optional advert hint: ignore (do not tear down existing binding)
      return;
    }
    this.store.set_base_url(handle, http_base_url);
  }

  private drop_handle(handle: Handle): void {
    const pid = this.pid_by_handle.get(handle);
    this.pid_by_handle.delete(handle);
    this.wire_down_by_handle.delete(handle);
    this.store_down_by_handle.delete(handle);
    this.down_since_ms_by_handle.delete(handle);
    this.store.unset_base_url(handle);

    if (pid && this.handle_by_pid.get(pid) === handle) {
      this.handle_by_pid.delete(pid);
    }
  }

  private ensure_dead_timer(): void {
    if (this.dead_timer) return;
    this.dead_timer = setInterval(() => this.sweep_dead(), 1000);
  }

  // continuous degraded-time policy:
  // - start timer on first degradation edge (wire_down/store_down becomes true)
  // - clear timer only when ALL internal lanes are recovered
  // - if degraded state persists for grace, emit PEER_UNREACHABLE
  private sweep_dead(): void {
    const now = Date.now();
    for (const [handle, since] of this.down_since_ms_by_handle) {
      if (now - since < this.dead_grace_ms) continue;

      const pid = this.pid_by_handle.get(handle);
      if (!pid) {
        this.down_since_ms_by_handle.delete(handle);
        continue;
      }

      // stale handle guard: if pid already rebound to a new handle, ignore old one
      if (this.handle_by_pid.get(pid) !== handle) {
        this.down_since_ms_by_handle.delete(handle);
        continue;
      }

      const wire_down = this.wire_down_by_handle.has(handle);
      const store_down = this.store_down_by_handle.has(handle);
      if (!wire_down && !store_down) continue;

      // CENTRAL internal lanes are private; externally this is a single link reachability decision.
      this.emit_unreachable_terminal(pid, "link_unreachable");
    }

    // stop timer if nothing to watch
    if (!this.down_since_ms_by_handle.size) {
      clearInterval(this.dead_timer);
      this.dead_timer = null;
    }
  }

  on(rx: RxFn): void {
    this.rx = rx;
  }

  on_event(ev: TransportEventFn): void {
    this.ev = ev;
  }

  send(dst: Peer, rid: RID, enc_hdr: Bytes, enc_body: Bytes | null, sig: Signature): void {
    const pid = dst.pid;
    if (this.unreachable_pid.has(pid)) {
      // terminal peer already declared for current binding epoch; wait for re-HELLO/rebind
      drop(`send_peer_unreachable pid=${pid}`);
      return;
    }
    const handle = this.handle_by_pid.get(pid);
    if (!handle) {
      // non-fatal: peer selection may race with HELLO completion / teardown
      drop(`send_no_handle pid=${pid}`);
      return;
    }
    const has_blob: 0 | 1 = enc_body && enc_body.byteLength ? 1 : 0;

    const msg: Message = {
      key: rid,
      has_blob,
      enc_hdr,
      sig,
    };

    // IMPORTANT: for has_blob=1, ensure PUT is observable before the header notify.
    if (has_blob) {
      void this.store
        .put(handle, rid, enc_body!)
        .then(() => {
          // stale handle guard: pid may have been rebound while PUT was in flight
          if (this.handle_by_pid.get(pid) !== handle) return;

          this.store_down_by_handle.delete(handle);
          if (!this.wire_down_by_handle.has(handle) && !this.store_down_by_handle.has(handle)) {
            this.down_since_ms_by_handle.delete(handle);
          }

          try {
            this.wire.send(handle, msg);
          } catch (err) {
            if (!(err instanceof NetworkFaultError)) throw err;
            this.on_wire_send_err(handle, pid, err);
          }
        })
        .catch((err) => {
          // stale handle guard: pid may have been rebound while PUT was in flight
          if (this.handle_by_pid.get(pid) !== handle) return;
          if (!(err instanceof NetworkFaultError)) throw err;
          this.on_store_put_err(handle, err);
        });
      return;
    }

    try {
      this.wire.send(handle, msg);
    } catch (err) {
      if (!(err instanceof NetworkFaultError)) throw err;
      this.on_wire_send_err(handle, pid, err);
    }
  }

  send_hello(dst_handle: Handle, rid: RID, enc_hdr: Bytes, sig: Signature): void {
    // HELLO path: handle-addressed. No store usage, no body by contract.
    const msg: Message = { key: rid, has_blob: 0, enc_hdr, sig };
    // HELLO: send to provided handle. plug routes purely by handle string.
    try {
      this.wire.send(dst_handle, msg);
    } catch (err) {
      if (!(err instanceof NetworkFaultError)) throw err;
      drop("hello_send_failed", err.errno);
    }
  }

  private async on_wire(src: Handle, msg: Message): Promise<void> {
    // - src is always a transport link handle (transport-owned opaque id; e.g. ws handle, rtc dc-id).
    // - overlay PID is not trusted until HELLO binds PID <-> Handle and crypto verifies.

    // unknown handle allowed ONLY for NET_HELLO
    if (!this.pid_by_handle.has(src)) {
      if (!this.is_pre_auth_allowed(msg.enc_hdr, msg.has_blob)) return;
    }

    const rid: RID = msg.key;

    if (!msg.has_blob) {
      this.rx?.(src, rid, msg.enc_hdr, null, msg.sig);
      return;
    }

    // Store is keyed by Handle.
    let blob: Bytes;
    try {
      blob = await this.store.get(src, msg.key);
      this.store_down_by_handle.delete(src);
      if (!this.wire_down_by_handle.has(src) && !this.store_down_by_handle.has(src)) {
        this.down_since_ms_by_handle.delete(src);
      }
    } catch (err) {
      if (!(err instanceof NetworkFaultError)) throw err;
      this.on_store_get_err(src, err);
      return;
    }
    if (!blob.byteLength) return;

    this.rx?.(src, rid, msg.enc_hdr, blob, msg.sig);
  }

  private on_wire_send_err(handle: Handle, pid: PID, err: unknown): void {
    if (this.handle_by_pid.get(pid) !== handle) return;
    if (err instanceof XputeError && err.errno === Errno.ENOENT) {
      drop("wire_send_no_client", err.errno);
      return;
    }
    if (!this.wire_down_by_handle.has(handle)) {
      this.wire_down_by_handle.add(handle);
      if (!this.down_since_ms_by_handle.has(handle)) this.down_since_ms_by_handle.set(handle, Date.now());
      this.ensure_dead_timer();
      this.ev?.({
        vec: Vector.PEER_UNSTABLE,
        handle,
        pid,
        reason: "wire_send_failed",
        errno: err instanceof XputeError ? err.errno : undefined,
      });
      if (err instanceof XputeError) drop("wire_send_failed", err.errno);
      else drop("wire_send_failed");
    }
  }

  private on_store_put_err(handle: Handle, err: unknown): void {
    if (!this.store_down_by_handle.has(handle)) {
      this.store_down_by_handle.add(handle);
      if (!this.down_since_ms_by_handle.has(handle)) this.down_since_ms_by_handle.set(handle, Date.now());
      this.ensure_dead_timer();
      const pid = this.pid_by_handle.get(handle);
      this.ev?.({
        vec: Vector.PEER_UNSTABLE,
        handle,
        pid,
        reason: "store_path_degraded_put",
        errno: err instanceof XputeError ? err.errno : undefined,
      });
      if (err instanceof XputeError) drop("store_put_degraded", err.errno);
      else drop("store_put_degraded");
    }
  }

  private on_store_get_err(handle: Handle, err: unknown): void {
    if (err instanceof XputeError && err.errno === Errno.ENOENT) return;
    if (!this.store_down_by_handle.has(handle)) {
      this.store_down_by_handle.add(handle);
      if (!this.down_since_ms_by_handle.has(handle)) this.down_since_ms_by_handle.set(handle, Date.now());
      this.ensure_dead_timer();
      const pid = this.pid_by_handle.get(handle);
      this.ev?.({
        vec: Vector.PEER_UNSTABLE,
        handle,
        pid,
        reason: "store_path_degraded_get",
        errno: err instanceof XputeError ? err.errno : undefined,
      });
      if (err instanceof XputeError) drop("store_get_degraded", err.errno);
      else drop("store_get_degraded");
    }
  }
}
