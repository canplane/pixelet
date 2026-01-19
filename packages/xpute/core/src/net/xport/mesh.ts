// @xpute/core/net/xport/mesh.ts

import type { Bytes } from "@xpute/spec/lib/kit/array.ts";
import { Errno } from "@xpute/spec/sys/errno.ts";
import { InvariantError, NetworkFaultError } from "@xpute/spec/sys/error.ts";

import { Vector } from "@xpute/spec/control/vec.ts";
import type { PID, Signature } from "@xpute/spec/net/crypto.ts";
import type { RID } from "@xpute/spec/rpc/rid.ts";
import type { Dial, Handle, MeshTransport as _MeshTransport, Peer, RxFn, TransportEvent, TransportEventFn } from "@xpute/spec/net/xport/mod.ts";
import { alloc_handle, TransportKind } from "@xpute/spec/net/xport/mod.ts";
import type { MeshAdvert, MeshDialSeed, MeshPlug } from "@xpute/spec/net/xport/mesh.ts";
import { split_dial } from "@xpute/spec/net/xport/mesh.ts";

import { sniff_wire_vec } from "@xpute/spec/rpc/wire_header.ts";

import { drop as _drop } from "../kit/drop.ts";
const drop = (why: string, errno?: Errno): void => _drop("mesh", { why, errno });

export class MeshTransport implements _MeshTransport {
  readonly kind = TransportKind.MESH;

  private rx: RxFn | null = null;
  private ev: TransportEventFn | null = null;

  private readonly handle_by_pid: Map<PID, Handle> = new Map();
  private readonly pid_by_handle: Map<Handle, PID> = new Map();
  private readonly seed_by_handle: Map<Handle, MeshDialSeed> = new Map();
  private readonly advert_by_handle: Map<Handle, MeshAdvert> = new Map();
  private readonly unreachable_pid: Set<PID> = new Set();

  constructor(private readonly plug: MeshPlug) {
    this.plug.on_hello_bootstrap((handle, rid, enc_hdr, sig) => {
      if (!this.seed_by_handle.has(handle) && !this.pid_by_handle.has(handle)) return;
      this.rx?.(handle, rid, enc_hdr, null, sig);
    });

    this.plug.on_event((ev) => {
      this.on_rtc_event(ev);
    });

    this.plug.on_packet((handle, rid, enc_hdr, enc_body, sig) => {
      if (!this.pid_by_handle.has(handle)) return;
      this.rx?.(handle, rid, enc_hdr, enc_body, sig);
    });
  }

  private is_known_handle(handle: Handle): boolean {
    return this.seed_by_handle.has(handle) || this.pid_by_handle.has(handle);
  }

  private current_pid_of_handle(handle: Handle): PID | undefined {
    return this.pid_by_handle.get(handle);
  }

  private on_rtc_event(ev: TransportEvent): void {
    switch (ev.vec) {
      case Vector.LINK_UP: {
        // Suppress: canonical LINK_UP emitted on bind_handle_pid()
        return;
      }
      case Vector.LINK_DOWN:
      case Vector.PEER_UNSTABLE: {
        if (!this.is_known_handle(ev.handle)) return;
        const pid = this.current_pid_of_handle(ev.handle);
        this.ev?.({ ...ev, pid: pid ?? ev.pid });
        return;
      }
      case Vector.PEER_UNREACHABLE: {
        this.emit_unreachable_terminal(ev.pid, ev.reason ?? "plug_unreachable", ev.errno);
        return;
      }
    }
  }

  private drop_handle_local(handle: Handle): void {
    const pid = this.pid_by_handle.get(handle);
    if (pid) {
      this.pid_by_handle.delete(handle);
      if (this.handle_by_pid.get(pid) === handle) this.handle_by_pid.delete(pid);
    }

    this.seed_by_handle.delete(handle);
    this.advert_by_handle.delete(handle);
  }

  private close_handle_terminal(handle: Handle): void {
    this.drop_handle_local(handle);

    void this.plug.close_handle(handle).catch((err) => {
      if (!(err instanceof NetworkFaultError)) throw err;
    });
  }

  private emit_unreachable_terminal(pid: PID, reason: string, errno?: Errno): void {
    const handle = this.handle_by_pid.get(pid);
    if (!handle) return;

    if (this.unreachable_pid.has(pid)) return;
    this.unreachable_pid.add(pid);

    this.close_handle_terminal(handle);

    this.ev?.({
      vec: Vector.PEER_UNREACHABLE,
      pid,
      reason,
      errno,
    });
  }

  private emit_unstable_hint(handle: Handle, pid: PID | undefined, reason: string, errno?: Errno): void {
    this.ev?.({
      vec: Vector.PEER_UNSTABLE,
      handle,
      pid,
      reason,
      errno,
    });
  }

  private on_hello_bootstrap_send_err(handle: Handle, err: NetworkFaultError, reason: string): void {
    if (!this.is_known_handle(handle)) return;
    if (err.errno === Errno.ENOTSUP) return;
    if (err.errno === Errno.ENOTCONN) return;
    this.emit_unstable_hint(handle, this.current_pid_of_handle(handle), reason, err.errno);
  }

  is_pre_auth_allowed(enc_hdr: Bytes, has_body: 0 | 1): boolean {
    if (has_body) return false;
    const vec = sniff_wire_vec(enc_hdr);
    return vec === Vector.NET_HELLO;
  }

  dial_prefixes(): readonly string[] {
    return ["rtc:", "turn:"];
  }

  private choose_advert_signaling_seed(advert?: MeshAdvert): MeshDialSeed | null {
    const fallback_dial = advert?.signal_dial ?? advert?.rendezvous_dial ?? null;
    if (!fallback_dial) return null;

    let seed: MeshDialSeed;
    try {
      seed = split_dial(fallback_dial);
    } catch {
      return null;
    }
    if (seed.scheme !== "rtc") return null;
    return seed;
  }

  private async upgrade_handle_bootstrap_seed(handle: Handle, seed_next: MeshDialSeed, reason_on_fail: string): Promise<void> {
    const seed_prev = this.seed_by_handle.get(handle);
    if (!seed_prev) return;

    this.seed_by_handle.set(handle, seed_next);
    try {
      await this.plug.seed(handle, seed_next);
    } catch (err) {
      if (this.seed_by_handle.get(handle) === seed_next) this.seed_by_handle.set(handle, seed_prev);

      if (err instanceof NetworkFaultError) {
        this.on_hello_bootstrap_send_err(handle, err, reason_on_fail);
        return;
      }
      throw err;
    }
  }

  private send_hello_bootstrap_via_seed(handle: Handle, rid: RID, enc_hdr: Bytes, sig: Signature, reason_on_fail: string): void {
    void this.plug.send_hello_bootstrap(handle, rid, enc_hdr, sig).catch((err) => {
      if (!(err instanceof NetworkFaultError)) throw err;
      this.on_hello_bootstrap_send_err(handle, err, reason_on_fail);
    });
  }

  async dial_seed(dial: Dial): Promise<Handle> {
    const handle = alloc_handle(this.kind);
    const seed = split_dial(dial);

    this.seed_by_handle.set(handle, seed);
    try {
      await this.plug.seed(handle, seed);
    } catch (err) {
      this.seed_by_handle.delete(handle);
      this.advert_by_handle.delete(handle);
      throw err;
    }

    return handle;
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
      drop(`send_peer_unreachable pid=${pid}`);
      return;
    }

    const handle = this.handle_by_pid.get(pid);
    if (!handle) {
      drop(`send_no_handle pid=${pid}`);
      return;
    }

    void this.plug.send_packet(handle, rid, enc_hdr, enc_body, sig).catch((err) => {
      if (!(err instanceof NetworkFaultError)) throw err;

      if (this.handle_by_pid.get(pid) !== handle) return;
      if (err.errno === Errno.ENOTSUP) return;
      if (err.errno === Errno.ENOTCONN) return;
      this.emit_unstable_hint(handle, pid, "packet_send_failed", err.errno);
    });
  }

  send_hello(dst_handle: Handle, rid: RID, enc_hdr: Bytes, sig: Signature): void {
    if (this.current_pid_of_handle(dst_handle)) return;

    const seed = this.seed_by_handle.get(dst_handle);
    if (!seed) {
      throw new NetworkFaultError(Errno.ENOENT, `mesh seed missing for handle: ${dst_handle}`);
    }

    if (seed.scheme === "rtc") {
      this.send_hello_bootstrap_via_seed(dst_handle, rid, enc_hdr, sig, "hello_bootstrap_failed");
      return;
    }

    // TURN seed: try advert fallback
    const advert = this.advert_by_handle.get(dst_handle);
    const seed2 = this.choose_advert_signaling_seed(advert);

    if (!seed2) {
      // No signaling path available for TURN-only seed
      drop(`send_hello_no_signaling handle=${dst_handle}`);
      throw new NetworkFaultError(Errno.ENOTCONN, "no signaling path for turn-only seed");
    }

    void this.upgrade_handle_bootstrap_seed(dst_handle, seed2, "fallback_signaling_seed_failed").then(() => {
      if (!this.is_known_handle(dst_handle)) return;
      if (this.current_pid_of_handle(dst_handle)) return;
      this.send_hello_bootstrap_via_seed(dst_handle, rid, enc_hdr, sig, "fallback_signaling_bootstrap_failed");
    });
  }

  bind_handle_pid(handle: Handle, pid: PID): void {
    const prev_pid = this.pid_by_handle.get(handle);
    if (prev_pid && prev_pid !== pid) throw new InvariantError(Errno.EEXIST, "handle->pid conflict");

    const prev_handle = this.handle_by_pid.get(pid);
    if (prev_pid === pid && prev_handle === handle) {
      this.unreachable_pid.delete(pid);
      return;
    }
    if (prev_handle && prev_handle !== handle) {
      this.close_handle_terminal(prev_handle);
    }

    this.pid_by_handle.set(handle, pid);
    this.handle_by_pid.set(pid, handle);
    this.unreachable_pid.delete(pid);
    this.ev?.({ vec: Vector.LINK_UP, handle, pid });
  }

  bind_handle_advert(handle: Handle, advert?: MeshAdvert): void {
    if (!this.is_known_handle(handle)) return;

    if (!advert) {
      this.advert_by_handle.delete(handle);
      try {
        this.plug.apply_advert(handle, undefined);
      } catch {}
      return;
    }

    if (!advert.rendezvous_dial && !advert.signal_dial && !(advert.ice_servers && advert.ice_servers.length)) {
      this.advert_by_handle.delete(handle);
      try {
        this.plug.apply_advert(handle, undefined);
      } catch {}
      return;
    }
    this.advert_by_handle.set(handle, {
      rendezvous_dial: advert.rendezvous_dial,
      signal_dial: advert.signal_dial,
      ice_servers: advert.ice_servers,
    });
    try {
      this.plug.apply_advert(handle, advert);
    } catch {}

    const cur = this.seed_by_handle.get(handle);
    if (!cur || cur.scheme !== "turn") return;

    const seed2 = this.choose_advert_signaling_seed(advert);
    if (!seed2) return;

    void this.upgrade_handle_bootstrap_seed(handle, seed2, "advert_seed_upgrade_failed");
  }
}
