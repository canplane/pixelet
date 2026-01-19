// @xpute/core/net/gateway.ts

import type { u32 } from "@xpute/spec/lib/kit/word.ts";
import type { Bytes } from "@xpute/spec/lib/kit/array.ts";
import * as encoding from "@xpute/spec/lib/kit/encoding.ts";
import { Errno } from "@xpute/spec/sys/errno.ts";
import { InvariantError, NetworkFaultError } from "@xpute/spec/sys/error.ts";

import type { Identity, IdentityKey, PID, Signature } from "@xpute/spec/net/crypto.ts";
import * as crypto from "@xpute/spec/net/crypto.ts";
import { Vector } from "@xpute/spec/control/vec.ts";
import type { XputeManifest } from "@xpute/spec/ipc/topology.ts";
import type { NetUplink } from "@xpute/spec/ipc/link.ts";
import type { SignalFrame } from "@xpute/spec/ipc/frame.ts";
import type { NodeSelector } from "@xpute/spec/rpc/node.ts";
import { generate_rid, type RID } from "@xpute/spec/rpc/rid.ts";
import { pack_wire_header, unpack_wire_header, type WireHeader } from "@xpute/spec/rpc/wire_header.ts";
import type { Dial, Handle, Peer, Transport, TransportEvent, TransportKind, Transports } from "@xpute/spec/net/xport/mod.ts";

import { PeerDirectory } from "@xpute/core/rpc/peer/directory.ts";
import { Hello } from "./hello/mod.ts";
import { make_hello_hooks } from "./hello/hooks.ts";

import { drop as _drop } from "./kit/drop.ts";
const drop = (why: string, errno?: Errno): void => _drop("net_gateway", { why, errno });

const BODY_NULL: u32 = 0xffffffff;
const MAX_BODY: u32 = 8 * 1024 * 1024; // 8 MiB (plain cap; optional)

export type NetGatewayBootOptions = {
  /**
   * COOKIE_SECRET_B64URL=<32-byte-random-base64url>
   * option 1. $ node -e "console.log(require('crypto').randomBytes(32).toString('base64url'))"
   * option 2. $ openssl rand -base64 32 | tr '+/' '-_' | tr -d '='
   *
   * Ops/dev notes:
   * - purpose: HELLO cookie HMAC secret (bootstrap anti-amplification / retry-cookie validation)
   * - recommended size: 32 random bytes (base64url-encoded)
   * - production: inject via secret manager / runtime secret, DO NOT commit to repo
   * - multi-instance: all API instances serving the same cluster must share the same secret
   * - rotation (current single-key policy): changing this invalidates in-flight HELLO cookies
   *   (typically short impact only; HELLO token TTL is short). dual-window rotation can be added later.
   * - if omitted/invalid, NetGateway falls back to local random (dev-friendly, but cross-restart
   *   and cross-instance stability is lost; not recommended for production)
   */
  hello_cookie_secret?: string;
};

// TODO(security/ops):
// - kill switch: outbound stop (global), per-xport stop, HELLO stop (pre-auth accept deny)
// - rate limit: inbound pre-auth HELLO path, post-auth packet path, per-pid replay/cache pressure
// - admission policy: optional temporary denylist/blocklist by handle/pid/xport (manual incident response)
// - observability: counters for recv_drop_reason (no_pid, non_hello_pre_auth, bad_sig, replay, decrypt_fail, parse_fail)
// - structured log consistency: keep short stable reason strings for grep/alerting
// - replay cache policy: tune ttl/max per env; add metrics for prune count / hit rate / evictions
// - body hash CPU budget: consider async batching / cap / sampling if large fanout becomes hot path
// - backpressure policy: define behavior when uplink is slow/full (drop / queue / signal)
// - xport health aggregation: summarize LINK_DOWN/UNSTABLE storms to reduce log spam
// - fault injection hooks (dev/test): bad sig / bad hdr / replay / decrypt fail path simulation
// - incident mode: verbose logging toggle for handshake/rx path (console.warn remains default for now)

export class NetGateway {
  private readonly dir: PeerDirectory = new PeerDirectory();

  // boot() completes these fields.
  // Access before boot is programmer error (state machine violation).
  // Public methods assume booted state unless explicitly documented otherwise.
  private identity!: Identity;
  private role_mask!: NodeSelector;
  private hello!: Hello;

  private booted = false;

  private readonly dialer_by_prefix: Map<string, Transport> = new Map();
  private readonly handle_pid: Map<string, PID> = new Map(); // key(kind,handle) -> pid

  private readonly replay_ttl_ms = 120_000;
  private readonly replay_max_per_peer = 1024;
  private readonly replay_seen_by_pid: Map<PID, Map<RID, number>> = new Map();

  constructor(
    private readonly uplink: NetUplink,
    private readonly xports: Readonly<Transports>,
    private readonly boot_opts: NetGatewayBootOptions = {},
  ) {}

  private handle_key(kind: TransportKind, handle: Handle): string {
    return `${kind}\0${handle}`;
  }

  private seen_replay(pid: PID, rid: RID): boolean {
    const now = Date.now();
    let m = this.replay_seen_by_pid.get(pid);
    if (!m) {
      m = new Map();
      this.replay_seen_by_pid.set(pid, m);
    }

    // ttl prune
    for (const [rid2, ts] of m) {
      if (now - ts > this.replay_ttl_ms) m.delete(rid2);
    }

    if (m.has(rid)) return true;

    // bounded per-peer cache (simple FIFO via Map insertion order)
    if (m.size >= this.replay_max_per_peer) {
      const it = m.keys().next();
      if (!it.done) m.delete(it.value);
    }

    m.set(rid, now);
    return false;
  }

  private bind_handle_pid(kind: TransportKind, handle: Handle, pid: PID): void {
    this.handle_pid.set(this.handle_key(kind, handle), pid);
    this.xports[kind].bind_handle_pid(handle, pid);
  }

  private unbind_handles_by_pid(pid: PID): void {
    for (const [k, v] of this.handle_pid) {
      if (v === pid) this.handle_pid.delete(k);
    }
  }

  private make_hello_cookie_secret(): Bytes {
    let hello_cookie_secret: Bytes;
    const s = this.boot_opts.hello_cookie_secret;
    const is_prod_server = __ENV__ !== "browser" && !__IS_DEV__;

    // Production hard-fail policy:
    // - require explicit stable shared secret
    // - do not silently fall back to local random (would break multi-instance/restart continuity)
    if (is_prod_server && !(typeof s === "string" && s.length)) throw new InvariantError(Errno.EINVAL, "missing hello_cookie_secret in production");

    if (typeof s === "string" && s.length) {
      try {
        hello_cookie_secret = encoding.b64url_to_bytes(s);
        if (!hello_cookie_secret.byteLength) {
          if (is_prod_server) throw new InvariantError(Errno.EINVAL, "invalid hello_cookie_secret (empty decode) in production");

          // invalid operational input (empty after decode) -> local random fallback.
          // keeps dev boot alive, but operators should treat this as misconfiguration.
          hello_cookie_secret = crypto.random_bytes(32);
          drop("hello_cookie_secret_empty_fallback_random");
        }
      } catch {
        if (is_prod_server) throw new InvariantError(Errno.EINVAL, "invalid hello_cookie_secret (parse failed) in production");

        // parse failure -> local random fallback (dev convenience).
        // production should prefer explicit fail-fast policy at app bootstrap if desired.
        hello_cookie_secret = crypto.random_bytes(32);
        drop("hello_cookie_secret_parse_failed_fallback_random");
      }
    } else {
      // no configured secret -> local random fallback.
      // safe for local dev, but breaks secret continuity across restart/replicas.
      hello_cookie_secret = crypto.random_bytes(32);
    }

    return hello_cookie_secret;
  }

  private on_xport_event(ev: TransportEvent): void {
    switch (ev.vec) {
      case Vector.LINK_UP:
      case Vector.LINK_DOWN: {
        // stop any handle-keyed HELLO retransmit state on link-down.
        if (ev.vec === Vector.LINK_DOWN) this.hello.drop_handle(ev.handle);
        // 힌트 이벤트: unregister 금지. 필요하면 host-visible signal만 추가.
        return;
      }
      case Vector.PEER_UNSTABLE: {
        // 힌트 이벤트: unregister 금지. (SDN / policy tuning hook)
        // 필요하면 host-visible signal로 승격.
        return;
      }
      case Vector.PEER_UNREACHABLE: {
        const pid = ev.pid;
        if (pid === this.identity.pid) return;
        if (!this.dir.reverse(pid)) return; // unknown or pre-auth tmp/seed pid
        this.unbind_handles_by_pid(pid);
        this.replay_seen_by_pid.delete(pid);
        this.dir.unregister(pid);
        return;
      }
    }
    // unreachable (discriminated union)
    throw new InvariantError(Errno.EINVAL, "unknown transport event");
  }

  private wire_xports(): void {
    // One-time callback wiring during boot(); callbacks assume initialized runtime fields.
    for (const xport of Object.values(this.xports)) {
      xport.on((src_handle, rid, enc_hdr, enc_body, sig) => void this.receive(xport.kind, src_handle, rid, enc_hdr, enc_body, sig));
      xport.on_event((ev) => void this.on_xport_event(ev));

      for (const p of xport.dial_prefixes()) {
        if (!this.dialer_by_prefix.has(p)) this.dialer_by_prefix.set(p, xport); // first-wins policy
      }
    }
  }

  private resolve_dialer(dial: Dial): Transport | null {
    // longest-prefix match (stable + deterministic)
    let best: Transport | null = null;
    let best_len = 0;
    for (const [p, d] of this.dialer_by_prefix) {
      if (p.length <= best_len) continue;
      if (!dial.startsWith(p)) continue;
      best = d;
      best_len = p.length;
    }
    return best;
  }

  /**
   * Dial bootstrap SSOT pipeline (seed boot + runtime dynamic connect)
   *
   * Internal path only. Precondition: boot() completed.
   *
   * Flow:
   * - resolve_dialer(dial)
   * - dial_seed(dial) -> handle
   * - HELLO SYN on (kind, handle)
   */
  private async connect_dial(dial: Dial): Promise<void> {
    const picked = this.resolve_dialer(dial);
    if (!picked) {
      drop(`connect_dial_no_dialer dial=${dial}`);
      return;
    }

    let handle: Handle;
    try {
      handle = await picked.dial_seed(dial);
    } catch (err) {
      if (!(err instanceof NetworkFaultError)) throw err;
      drop("connect_dial_dial_seed_failed", err.errno);
      return;
    }

    try {
      this.hello.send_syn(picked.kind, handle);
    } catch (err) {
      if (!(err instanceof NetworkFaultError)) throw err;
      drop("connect_dial_hello_syn_failed", err.errno);
    }
  }

  async init(net_spec: XputeManifest["net"]): Promise<void> {
    if (this.booted) throw new InvariantError(Errno.EEXIST, "already booted");
    // boot() is the only state-machine entry that initializes runtime wiring/identity.
    // After commit (booted=true), callbacks/public send path may assume initialized fields.

    // ---- identity + local role ----
    this.identity = await crypto.generate_identity();
    this.role_mask = net_spec.role;

    // ---- hello cookie secret (env base64url -> bytes, else local random fallback) ----
    // Operational caution:
    // - prod should provide a stable shared secret across API replicas/restarts
    // - local random fallback is acceptable for single-process dev/test only
    const hello_cookie_secret: Bytes = this.make_hello_cookie_secret();

    // HELLO promotion hook wiring (manifest/net_spec-aware, boot-time)
    this.hello = new Hello(
      this.identity,
      this.role_mask,
      make_hello_hooks(this.xports, this.dir, (kind, handle, pid) => this.bind_handle_pid(kind, handle, pid), net_spec),
      hello_cookie_secret,
    );

    // Inbound/event path becomes reachable only after wiring.
    // Commit booted after wiring so state-machine and callback reachability align.
    this.wire_xports();

    // state machine commit point: boot completed
    this.booted = true;

    // seed bootstrap uses the same Dial->HELLO pipeline as runtime dynamic connect
    for (const dial of net_spec.seeds) await this.connect_dial(dial);
  }

  /**
   * Outbound: Host -> Network
   *
   * Public API.
   * Precondition: boot() completed (initialized identity/hello/xport wiring).
   * Pre-boot call is programmer error (state machine violation).
   */
  async send(frame: SignalFrame): Promise<void> {
    const my_sign_priv: IdentityKey = this.identity.sign_priv;
    const my_dh_priv: IdentityKey = this.identity.dh_priv;

    const fanout: u32 | undefined = frame.opts.fanout;
    const dst_sel: NodeSelector = frame.sel;

    const rid: RID = frame.rid ?? generate_rid(frame.vec, frame.args);

    // (1) pack header (plain)
    const plain_hdr: WireHeader = pack_wire_header(frame);

    const peers: Peer[] = this.dir.forward(dst_sel, fanout);
    for (const peer of peers) {
      const xport: Transport = this.xports[peer.kind];

      // --- hdr/body split crypto ---
      // NOTE: peer must expose peer.sign_pub, peer.dh_pub
      const enc_hdr: Bytes = await crypto.aead_encrypt_hdr(my_dh_priv, peer.dh_pub, rid, plain_hdr);

      let enc_body: Bytes | null = null;
      let body_len: u32 = BODY_NULL;
      let body_hash32: Bytes | null = null;

      const body = frame.body;
      if (body && body.byteLength) {
        if (body.byteLength > MAX_BODY) {
          // non-fatal: frame rejected for this send attempt
          drop(`send_body_too_big n=${body.byteLength}`);
          continue;
        }

        enc_body = await crypto.aead_encrypt_body(my_dh_priv, peer.dh_pub, rid, body);
        body_len = enc_body.byteLength;
        body_hash32 = await crypto.hash256(enc_body);
      }

      const sig: Signature = await crypto.sign_packet(my_sign_priv, rid, enc_hdr, body_len, body_hash32);

      // transport handles mux/demux (ws/http, outbox/pull, etc.)
      try {
        xport.send(peer, rid, enc_hdr, enc_body, sig);
      } catch (err) {
        if (!(err instanceof NetworkFaultError)) throw err;
        // immediate local failure only; transport liveness policy/event path handles degradation
        drop("send_xport_send_failed", err.errno);
      }
    }
  }

  /**
   * Inbound: Network -> Host (transport already assembled hdr/body if split)
   *
   * Internal callback path only (wired from wire_xports()).
   * Not a public state-machine entry; assumes boot-time initialization completed.
   */
  private async receive(kind: TransportKind, src_handle: Handle, rid: RID, enc_hdr: Bytes, enc_body: Bytes | null, sig: Signature): Promise<void> {
    // NOTE: src_handle is transport-provided link identity (opaque Handle).
    //       Any associated PID is untrusted until HELLO + crypto verification.
    //       Do not trust peer identity until the peer is resolved/registered.
    const pid = this.handle_pid.get(this.handle_key(kind, src_handle));
    if (!pid) {
      // pre-register path: only accept NET_HELLO in *plaintext* wire header.
      // NOTE: enc_hdr is plaintext in this path (transport allowlist), despite the name.
      // NOTE: peer identity is unknown here. Only HELLO is allowed on unknown handles.
      // HELLO + signature/key checks will later bind a real PID into PeerDirectory.
      const wire_hdr = enc_hdr;

      const frame = unpack_wire_header(wire_hdr, 0n, rid);
      if (!frame) return;
      if (frame.vec !== Vector.NET_HELLO) return;
      if (enc_body && enc_body.byteLength) return; // hello has no body
      try {
        await this.hello.on_receive(kind, src_handle, frame, sig);
      } catch (err) {
        if (!(err instanceof NetworkFaultError)) throw err;
        drop("receive_hello_failed", err.errno);
      }
      return;
    }

    const resolution: [NodeSelector, Peer] | undefined = this.dir.reverse(pid);
    if (!resolution) return; // handle->pid exists but peer not registered yet (race); drop

    const my_dh_priv: IdentityKey = this.identity.dh_priv;

    const [src_vid, peer] = resolution;

    // verify signature first (binds rid + enc_hdr + enc_body(len/hash))
    let body_len: u32 = BODY_NULL;
    let body_hash32: Bytes | null = null;

    if (enc_body && enc_body.byteLength) {
      body_len = enc_body.byteLength;
      body_hash32 = await crypto.hash256(enc_body);
    }

    const ok = await crypto.verify_packet(peer.sign_pub, sig, rid, enc_hdr, body_len, body_hash32);
    if (!ok) {
      // non-fatal: peer/packet fault
      drop(`receive_bad_signature pid=${pid}`);
      return;
    }

    if (this.seen_replay(pid, rid)) {
      // non-fatal: duplicate packet replay
      drop(`receive_replay_dropped pid=${pid} rid=${rid}`);
      return;
    }

    // decrypt header -> parse
    let plain_hdr: Bytes;
    try {
      plain_hdr = await crypto.aead_decrypt_hdr(my_dh_priv, peer.dh_pub, rid, enc_hdr);
    } catch {
      // non-fatal: peer/packet fault
      drop(`receive_hdr_decrypt_failed pid=${pid}`);
      return;
    }

    const frame = unpack_wire_header(plain_hdr, src_vid, rid);
    if (!frame) {
      // non-fatal: peer/packet fault
      drop(`receive_corrupted_header pid=${pid}`);
      return;
    }

    // decrypt body if present
    if (enc_body && enc_body.byteLength) {
      try {
        frame.body = await crypto.aead_decrypt_body(my_dh_priv, peer.dh_pub, rid, enc_body);
      } catch {
        // non-fatal: peer/packet fault
        drop(`receive_body_decrypt_failed pid=${pid}`);
        return;
      }
    } else {
      frame.body = null;
    }

    this.uplink.send(frame);
  }
}
