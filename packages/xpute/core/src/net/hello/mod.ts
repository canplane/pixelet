// @xpute/core/net/hello/mod.ts

import type { i32, u64 } from "@xpute/spec/lib/kit/word.ts";
import { type Bytes, is_bytes } from "@xpute/spec/lib/kit/array.ts";
import type { UUID } from "@xpute/spec/lib/kit/encoding.ts";
import * as encoding from "@xpute/spec/lib/kit/encoding.ts";
import type { Stream } from "@xpute/spec/io/scalar.ts";
import * as scalar from "@xpute/spec/io/scalar.ts";
import { TLVReader, TLVWriter } from "@xpute/spec/io/marshal/tlv.ts";
import { Errno } from "@xpute/spec/sys/errno.ts";
import { InvariantError, NetworkFaultError } from "@xpute/spec/sys/error.ts";

import { FrameKind, type SignalFrame } from "@xpute/spec/ipc/frame.ts";
import type { NodeSelector } from "@xpute/spec/rpc/node.ts";
import type { Identity, PID, Signature } from "@xpute/spec/net/crypto.ts";
import * as crypto from "@xpute/spec/net/crypto.ts";
import { generate_rid, type RID } from "@xpute/spec/rpc/rid.ts";

import { Vector } from "@xpute/spec/control/vec.ts";
import type { AdvertByKind, Handle, PeerByKind } from "@xpute/spec/net/xport/mod.ts";
import { TransportKind } from "@xpute/spec/net/xport/mod.ts";
import { pack_wire_header } from "@xpute/spec/rpc/wire_header.ts";

import type { HelloHooks } from "./hooks.ts";

import { drop as _drop } from "../kit/drop.ts";
const drop = (why: string, errno?: Errno): void => _drop("hello", { why, errno });

const now_ms_u64 = (): u64 => BigInt(Date.now());
const now_s_u64 = (): u64 => now_ms_u64() / 1000n;

// ---- HELLO (2-RTT, cookie retry) ----
// TODO(security/ops):
// - rate limit (pre-auth): per-handle / per-source / per-time-window for HELLO_SYN, HELLO_RETRY, HELLO_ACK
// - backoff policy: consider jitter on HELLO retx to avoid synchronization bursts
// - duplicate suppression (pre-auth): short-lived cache for repeated SYN/RETRY storms
// - telemetry counters: hello_syn_rx / retry_tx / ack_rx / ok_rx / bad_cookie / bad_pid / bad_sig / ts_skew
// - log normalization: keep "HELLO bad X" message family stable for grep/alerting
// - log sampling (optional): noisy failures (bad_cookie, bad_sig) may need sample/drop policy under attack
// - source attribution (if transport can provide): enrich logs with remote addr hint without breaking handle opacity
// - cookie rotation: support active+previous secret window (grace rotation) for zero-downtime key rotation
// - secret provenance: production hard-fail option when cookie_secret missing (currently warn + may fail handshake)
// - replay/throttle coupling: pre-auth retry pacing guard to avoid CPU-amplification via HMAC/sign verify pressure
// - verify ordering review: maintain cheapest checks first (shape/size -> cookie -> ts -> pid -> sig)
// - malformed TLV observability (dev): optional debug counter only, avoid verbose logs for parser trash
// - advert.dial validation: optional scheme allowlist / length cap / canonicalization before bind
//   (dial hint vs plug-local resolved URL boundary must remain CENTRAL-transport-owned)
// - role_mask policy: optional allowlist/masking for externally accepted roles during bootstrap
// - bootstrap timestamp policy: tune skew by environment (dev wider, prod tighter) and document rationale
// - clock health hint: if repeated ts_skew, emit single summarized warning suggesting clock sync/NTP check
// - hello state expiry: reap stale hello_cnonce_by_handle / hello_ack_by_handle on long-lived handle leaks
// - handshake timeout surface: explicit terminal callback/event for HELLO give-up (observability + UI)
// - attack drill: document expected behavior under SYN flood / bad-cookie flood / bad-sig flood
// - fuzz tests: TLV decode_step + HELLO args permutations (length/type/order corruption)
// - property tests: token encode/verify invariants, handle binding invariants, timestamp window boundaries
// - metrics export seam (future): keep console.warn now, but preserve stable event names for later logger/metrics sink

const HELLO_SYN = 0;
const HELLO_RETRY = 1;
const HELLO_ACK = 2;
const HELLO_OK = 3;
type HelloStep = typeof HELLO_SYN | typeof HELLO_RETRY | typeof HELLO_ACK | typeof HELLO_OK;

// Retransmit/ACK-stop state is keyed by Handle (not PID).

const HELLO_RETX_MS = 250;
const HELLO_RETX_MAX = 8;
const HELLO_ACK_RETX_MS = 250;
const HELLO_ACK_RETX_MAX = 8;

const HELLO_CNONCE_LEN = 16;
const HELLO_SNONCE_LEN = 16;
const HELLO_MAC_LEN = 32;
const HELLO_TOKEN_LEN = 8 + HELLO_SNONCE_LEN + HELLO_MAC_LEN; // exp(u64) + snonce + mac
const HELLO_TTL_S: u64 = 30n;
const HELLO_HANDLEHASH_LEN = 32; // sha256
const HELLO_BOOT_TS_SKEW_MS: u64 = 5n * 60n * 1000n; // ±5 min

const MAX_HELLO_ARGS = 256; // hard cap (DoS guard)

// pre-auth global rate cap (simple window counter)
// - 목적: SYN flood / bad-cookie flood에서 CPU 스파이크 방지
// - NOTE: src attribution이 없으니 전역만 둔다 (추후 transport가 addr hint 주면 per-src로 확장)
const HELLO_RL_WINDOW_MS = 1000;
const HELLO_RL_BURST = 200; // allow up to N hello frames per second (tune per env)

// state caps (DoS guard)
const MAX_HELLO_CNONCE_LIVE = 4096;
const MAX_HELLO_ACK_LIVE = 4096;

type RetryCtl = {
  is_live: () => boolean;
  fire: () => void | Promise<void>;
  on_fault?: (err: NetworkFaultError) => void;
  max_retry: i32;
  base_ms: i32;
};

function schedule_bounded_backoff_retry(ctl: RetryCtl): void {
  let n = 0;

  const tick = () => {
    if (!ctl.is_live()) return;
    if (n >= ctl.max_retry) return;
    n++;

    const ms = ctl.base_ms << Math.min(n - 1, 3); // 250,500,1000,2000,2000...
    setTimeout(async () => {
      if (!ctl.is_live()) return;
      try {
        await ctl.fire();
      } catch (err) {
        if (!(err instanceof NetworkFaultError)) throw err;
        ctl.on_fault?.(err);
      }
      tick();
    }, ms);
  };

  tick();
}

// HELLO handshake engine used by NetGateway-owned transport bootstrap/auth path.
// Usage contract:
// - NetGateway constructs/owns this instance after local identity/role is ready.
// - NetGateway drives outbound SYN and inbound HELLO dispatch.
// - Not guarded for arbitrary lifecycle misuse by unrelated callers.
export class Hello {
  private readonly hello_cnonce_by_handle: Map<Handle, Bytes> = new Map();

  private readonly hello_ack_by_handle: Map<Handle, { kind: TransportKind; token: Bytes; cnonce: Bytes }> = new Map();

  // global RL state (per Hello instance)
  private rl_ts_ms = 0;
  private rl_n = 0;

  private allow_pre_auth(): boolean {
    const now = Date.now();
    if (now - this.rl_ts_ms >= HELLO_RL_WINDOW_MS) {
      this.rl_ts_ms = now;
      this.rl_n = 0;
    }
    if (this.rl_n >= HELLO_RL_BURST) return false;
    this.rl_n++;
    return true;
  }

  private ts_in_window(timestamp_ms: u64): boolean {
    const now: u64 = now_ms_u64();
    const skew: u64 = HELLO_BOOT_TS_SKEW_MS;
    if (timestamp_ms < now - skew) return false;
    if (timestamp_ms > now + skew) return false;
    return true;
  }

  constructor(
    private readonly identity: Identity,
    private readonly role_mask: NodeSelector,
    private readonly hooks: HelloHooks,
    private readonly hello_cookie_secret: Bytes,
  ) {}

  // transport handle is gone / unreachable: stop any in-flight HELLO retransmit state.
  // Handle-keyed by contract (pre-auth + handshake phases).
  drop_handle(handle: Handle): void {
    this.hello_cnonce_by_handle.delete(handle);
    this.hello_ack_by_handle.delete(handle);
  }

  private decode_step(args: Bytes): { step: HelloStep; it: unknown[] } | null {
    if (args.byteLength > MAX_HELLO_ARGS) return null;
    try {
      const r = new TLVReader(args);
      const it = r.read_all();
      if (it.length < 1) return null;
      const step = it[0];
      if (typeof step !== "number") return null;
      if (step !== HELLO_SYN && step !== HELLO_RETRY && step !== HELLO_ACK && step !== HELLO_OK) return null;
      return { step: step as HelloStep, it };
    } catch {
      return null;
    }
  }

  // ============ Helpers ============

  private rand_bytes(n: i32): Bytes | null {
    const out: Bytes = new Uint8Array(n);
    const grv = globalThis.crypto?.getRandomValues;
    if (!grv) return null;
    grv.call(globalThis.crypto, out);
    return out;
  }

  private async handle_id_bytes(kind: TransportKind, src_handle: Handle): Promise<Bytes> {
    const kb = encoding.te.encode(kind);
    const hb = encoding.te.encode(src_handle);
    const msg = new Uint8Array(kb.byteLength + 1 + hb.byteLength);
    msg.set(kb, 0);
    msg[kb.byteLength] = 0;
    msg.set(hb, kb.byteLength + 1);
    return await crypto.hash256(msg);
  }

  private hello_send_plain(kind: TransportKind, dst_handle: Handle, args: Bytes, sig: Signature): void {
    const frame = this.hello_frame(args);
    if (!frame) throw new InvariantError(Errno.E2BIG, "hello_frame failed");
    const rid: RID = generate_rid(frame.vec, frame.args);
    frame.rid = rid;
    const plain_hdr = pack_wire_header(frame);
    this.hooks.send_hello_plain(kind, dst_handle, rid, plain_hdr, sig);
  }

  private send_syn_once(kind: TransportKind, dst_handle: Handle, cnonce: Bytes): void {
    const w = new TLVWriter();
    w.u8(HELLO_SYN);
    w.bytes(cnonce);
    this.hello_send_plain(kind, dst_handle, w.finish(), new Uint8Array(0));
  }

  private async make_hello_cookie(kind: TransportKind, src_handle: Handle, cnonce: Bytes): Promise<Bytes | null> {
    const exp: u64 = now_s_u64() + HELLO_TTL_S;
    const snonce: Bytes | null = this.rand_bytes(HELLO_SNONCE_LEN);
    if (!snonce) return null;

    const handle_hash = await this.handle_id_bytes(kind, src_handle);
    if (handle_hash.byteLength !== HELLO_HANDLEHASH_LEN) return null;

    const msg: Bytes = new Uint8Array(8 + HELLO_SNONCE_LEN + HELLO_CNONCE_LEN + HELLO_HANDLEHASH_LEN);
    const c_msg: Stream = scalar.stream(msg);
    scalar.putu64(c_msg, exp);
    scalar.write(c_msg, snonce, HELLO_SNONCE_LEN);
    scalar.write(c_msg, cnonce, HELLO_CNONCE_LEN);
    scalar.write(c_msg, handle_hash, HELLO_HANDLEHASH_LEN);

    const mac: Bytes = await crypto.hmac256(this.hello_cookie_secret, msg);
    const tok: Bytes = new Uint8Array(HELLO_TOKEN_LEN);
    const c_tok: Stream = scalar.stream(tok);
    scalar.putu64(c_tok, exp);
    scalar.write(c_tok, snonce, HELLO_SNONCE_LEN);
    scalar.write(c_tok, mac, HELLO_MAC_LEN);
    return tok;
  }

  private async verify_hello_cookie(kind: TransportKind, src_handle: Handle, cnonce: Bytes, token: Bytes): Promise<boolean> {
    if (token.byteLength !== HELLO_TOKEN_LEN) return false;

    const r: Stream = scalar.stream(token);
    const exp = scalar.getu64(r);
    const snonce = token.subarray(r.off, r.off + HELLO_SNONCE_LEN);
    r.off += HELLO_SNONCE_LEN;
    const mac = token.subarray(r.off, r.off + HELLO_MAC_LEN);
    r.off += HELLO_MAC_LEN;

    const now: u64 = now_s_u64();
    if (now > exp) return false;

    const handle_hash = await this.handle_id_bytes(kind, src_handle);
    if (handle_hash.byteLength !== HELLO_HANDLEHASH_LEN) return false;

    const msg: Bytes = new Uint8Array(8 + HELLO_SNONCE_LEN + HELLO_CNONCE_LEN + HELLO_HANDLEHASH_LEN);
    const c: Stream = scalar.stream(msg);
    scalar.putu64(c, exp);
    scalar.write(c, snonce, HELLO_SNONCE_LEN);
    scalar.write(c, cnonce, HELLO_CNONCE_LEN);
    scalar.write(c, handle_hash, HELLO_HANDLEHASH_LEN);

    const mac2 = await crypto.hmac256(this.hello_cookie_secret, msg);
    return encoding.ct_eq(mac, mac2.subarray(0, HELLO_MAC_LEN));
  }

  private hello_frame(args: Bytes): SignalFrame | null {
    if (args.byteLength > MAX_HELLO_ARGS) return null;
    return {
      kind: FrameKind.SIGNAL,
      tx_id: 0,
      errno: 0,
      sel: 0n,
      rid: undefined as any,
      vec: Vector.NET_HELLO,
      flags: 0,
      opts: {},
      args,
      body: null,
    };
  }

  // ============ Outbound ============

  /**
   * NetGateway outbound bootstrap path only (seed/runtime dial -> HELLO SYN).
   * Not intended as an arbitrary external lifecycle entry.
   *
   * @throws NetworkFaultError
   */
  send_syn(kind: TransportKind, dst_handle: Handle): void {
    if (this.hello_cnonce_by_handle.size >= MAX_HELLO_CNONCE_LIVE) throw new NetworkFaultError(Errno.EAGAIN, "hello cnonce table full");

    const cnonce: Bytes | null = this.rand_bytes(HELLO_CNONCE_LEN);
    if (!cnonce) throw new NetworkFaultError(Errno.EIO, "rand_bytes failed");
    this.hello_cnonce_by_handle.set(dst_handle, cnonce);

    console.debug("hello: syn:send", { kind, handle: dst_handle });

    const send_once = (): void => this.send_syn_once(kind, dst_handle, cnonce);

    // first shot
    try {
      send_once();
    } catch (err) {
      this.hello_cnonce_by_handle.delete(dst_handle);
      throw err;
    }

    // retx until we get RETRY (which deletes/overwrites cnonce entry) or we give up
    schedule_bounded_backoff_retry({
      is_live: () => this.hello_cnonce_by_handle.get(dst_handle) === cnonce,
      fire: () => send_once(), // best-effort retx
      max_retry: HELLO_RETX_MAX,
      base_ms: HELLO_RETX_MS,
    });
  }

  private send_retry(kind: TransportKind, dst_handle: Handle, token: Bytes): void {
    const w = new TLVWriter();
    w.u8(HELLO_RETRY);
    w.bytes(token);
    console.debug("hello: retry:send", { kind, handle: dst_handle });
    this.hello_send_plain(kind, dst_handle, w.finish(), new Uint8Array(0));
  }

  private async send_ack(kind: TransportKind, dst_handle: Handle, token: Bytes, cnonce: Bytes): Promise<void> {
    const { pid, sign_priv, sign_pub, dh_pub } = this.identity;
    const timestamp_ms: u64 = now_ms_u64();
    const pid_bin: Bytes = encoding.b64url_to_bytes(pid); // 16
    const advert_bin: Bytes = this.hooks.encode_advert(kind);

    const w = new TLVWriter();
    w.u8(HELLO_ACK);
    w.bytes(token);
    w.bytes(cnonce);
    w.u64(this.role_mask);
    w.u64(timestamp_ms);
    w.bytes(pid_bin);
    w.bytes(sign_pub);
    w.bytes(dh_pub);
    w.bytes(advert_bin);
    const args = w.finish();

    const sig: Signature = await crypto.bootstrap_sign(sign_priv, pid, sign_pub, dh_pub, timestamp_ms);
    console.debug("hello: ack:send", { kind, handle: dst_handle, pid });
    this.hello_send_plain(kind, dst_handle, args, sig);
  }

  private async send_ok(kind: TransportKind, dst_handle: Handle): Promise<void> {
    const { pid, sign_priv, sign_pub, dh_pub } = this.identity;
    const timestamp_ms: u64 = now_ms_u64();
    const pid_bin: Bytes = encoding.b64url_to_bytes(pid); // 16
    const advert_bin: Bytes = this.hooks.encode_advert(kind);

    const w = new TLVWriter();
    w.u8(HELLO_OK);
    w.u64(this.role_mask);
    w.u64(timestamp_ms);
    w.bytes(pid_bin);
    w.bytes(sign_pub);
    w.bytes(dh_pub);
    w.bytes(advert_bin);
    const args = w.finish();

    const sig: Signature = await crypto.bootstrap_sign(sign_priv, pid, sign_pub, dh_pub, timestamp_ms);
    console.debug("hello: ok:send", { kind, handle: dst_handle, pid });
    this.hello_send_plain(kind, dst_handle, args, sig);
  }

  private async decode_verify_auth<K extends TransportKind>(
    step: typeof HELLO_ACK | typeof HELLO_OK,
    kind: K,
    src_handle: Handle,
    it: unknown[],
    sig: Signature,
  ): Promise<PeerByKind[K] | null> {
    if (sig.byteLength === 0) return null;

    let role_mask: unknown;
    let timestamp_ms: unknown;
    let pid_bin: unknown;
    let sign_pub: unknown;
    let dh_pub: unknown;
    let advert_bin: unknown;

    if (step === HELLO_ACK) {
      if (it.length !== 9) return null;

      const token = it[1];
      const cnonce = it[2];
      role_mask = it[3];
      timestamp_ms = it[4];
      pid_bin = it[5];
      sign_pub = it[6];
      dh_pub = it[7];
      advert_bin = it[8];

      if (!is_bytes(token) || token.byteLength !== HELLO_TOKEN_LEN) return null;
      if (!is_bytes(cnonce) || cnonce.byteLength !== HELLO_CNONCE_LEN) return null;

      const ok: boolean = await this.verify_hello_cookie(kind, src_handle, cnonce, token);
      if (!ok) {
        console.debug("hello: reject", { kind, handle: src_handle, step, why: "bad_cookie" });
        drop("rx_ack_bad_cookie");
        return null;
      }
    } else {
      if (it.length !== 7) return null;

      role_mask = it[1];
      timestamp_ms = it[2];
      pid_bin = it[3];
      sign_pub = it[4];
      dh_pub = it[5];
      advert_bin = it[6];
    }

    if (typeof role_mask !== "bigint") return null;
    if (typeof timestamp_ms !== "bigint") return null;
    if (!is_bytes(pid_bin) || pid_bin.byteLength !== 16) return null;
    if (!is_bytes(sign_pub) || sign_pub.byteLength !== 32) return null;
    if (!is_bytes(dh_pub) || dh_pub.byteLength !== 32) return null;
    if (!is_bytes(advert_bin)) return null;

    if (!this.ts_in_window(timestamp_ms)) {
      console.debug("hello: reject", { kind, handle: src_handle, step, why: "ts_skew" });
      drop(step === HELLO_ACK ? "rx_ack_ts_skew" : "rx_ok_ts_skew");
      return null;
    }

    const pid: PID = encoding.uuid_to_b64url(pid_bin as UUID);
    if (!encoding.is_pid_b64url_16(pid, pid_bin)) {
      console.debug("hello: reject", { kind, handle: src_handle, step, why: "bad_pid" });
      drop(step === HELLO_ACK ? "rx_ack_bad_pid" : "rx_ok_bad_pid");
      return null;
    }

    const ok: boolean = await crypto.bootstrap_verify(pid, sign_pub, dh_pub, timestamp_ms, sig);
    if (!ok) {
      console.debug("hello: reject", { kind, handle: src_handle, step, why: "bad_bootstrap_sig" });
      drop(step === HELLO_ACK ? "rx_ack_bad_bootstrap_sig" : "rx_ok_bad_bootstrap_sig");
      return null;
    }

    const advert: AdvertByKind[K] = this.hooks.decode_advert(kind, advert_bin);
    return { pid, role_mask, sign_pub, dh_pub, kind, advert } as PeerByKind[K];
  }

  // ============ Inbound ============

  /**
   * NetGateway HELLO dispatch path only (from NetGateway.receive pre-auth branch).
   * Assumes NetGateway-owned lifecycle/boot ordering contract.
   *
   * @throws NetworkFaultError
   */
  async on_receive(kind: TransportKind, src_handle: Handle, frame: SignalFrame, sig: Signature): Promise<void> {
    // global pre-auth rate cap (cheap guard)
    if (!this.allow_pre_auth()) {
      drop("hello_rate_limited");
      return;
    }

    const dec = this.decode_step(frame.args);
    if (!dec) return;
    const { step, it } = dec;

    console.debug("hello: recv", { kind, handle: src_handle, rid: frame.rid, step });

    switch (step) {
      // -------- server: SYN -> RETRY(cookie) --------
      case HELLO_SYN: {
        if (it.length !== 2) return;
        const cnonce = it[1];
        if (!is_bytes(cnonce) || cnonce.byteLength !== HELLO_CNONCE_LEN) return;

        const tok = await this.make_hello_cookie(kind, src_handle, cnonce);
        if (!tok) throw new NetworkFaultError(Errno.EIO, "make_hello_cookie failed");
        console.debug("hello: syn:recv", { kind, handle: src_handle });
        this.send_retry(kind, src_handle, tok);
        return;
      }

      // -------- client: RETRY -> ACK(identity+sig) --------
      case HELLO_RETRY: {
        if (it.length !== 2) return;
        const token = it[1];
        if (!is_bytes(token) || token.byteLength !== HELLO_TOKEN_LEN) return;

        const cnonce = this.hello_cnonce_by_handle.get(src_handle);
        if (!cnonce) return;
        this.hello_cnonce_by_handle.delete(src_handle);

        console.debug("hello: retry:recv", { kind, handle: src_handle });

        // first ACK
        await this.send_ack(kind, src_handle, token, cnonce);

        // schedule ACK retransmit until OK arrives (or give up)
        if (this.hello_ack_by_handle.size >= MAX_HELLO_ACK_LIVE) {
          drop("hello_ack_table_full");
          return;
        }
        this.hello_ack_by_handle.set(src_handle, { kind, token, cnonce });

        schedule_bounded_backoff_retry({
          is_live: () => this.hello_ack_by_handle.has(src_handle),
          fire: async () => {
            const st2 = this.hello_ack_by_handle.get(src_handle);
            if (!st2) return;
            await this.send_ack(st2.kind, src_handle, st2.token, st2.cnonce);
          },
          on_fault: () => {
            // silent: xport event path owns degradation observability
          },
          max_retry: HELLO_ACK_RETX_MAX,
          base_ms: HELLO_ACK_RETX_MS,
        });
        return;
      }

      // -------- server: ACK(cookie ok -> verify sig -> register -> OK) --------
      case HELLO_ACK: {
        const auth = await this.decode_verify_auth(HELLO_ACK, kind, src_handle, it, sig);
        if (!auth) return;

        console.debug("hello: ack:verified", { kind, handle: src_handle, pid: auth.pid });

        this.hooks.apply_advert(src_handle, kind, auth.advert);
        this.hooks.on_auth_peer(kind, src_handle, auth);

        console.debug("hello: peer:bound", { kind, handle: src_handle, pid: auth.pid });

        return await this.send_ok(kind, src_handle);
      }

      // -------- client: OK(verify server sig -> bind handle -> register) --------
      case HELLO_OK: {
        const auth = await this.decode_verify_auth(HELLO_OK, kind, src_handle, it, sig);
        if (!auth) return;

        this.hello_ack_by_handle.delete(src_handle);

        console.debug("hello: ok:verified", { kind, handle: src_handle, pid: auth.pid });

        this.hooks.apply_advert(src_handle, kind, auth.advert);
        this.hooks.on_auth_peer(kind, src_handle, auth);
        console.debug("hello: peer:bound", { kind, handle: src_handle, pid: auth.pid });
        return;
      }

      default:
        return;
    }
  }
}
