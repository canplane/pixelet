// @pxl/web/xpute/plug/rtc.ts - Browser WebRTC implementation

import { io, type Socket } from "socket.io-client";

import type { u32 } from "@xpute/spec/lib/kit/word.ts";
import { U32 } from "@xpute/spec/lib/kit/word.ts";
import type { Bytes } from "@xpute/spec/lib/kit/array.ts";
import * as encoding from "@xpute/spec/lib/kit/encoding.ts";
import { Errno } from "@xpute/spec/sys/errno.ts";
import { NetworkFaultError } from "@xpute/spec/sys/error.ts";

import { Vector } from "@xpute/spec/control/vec.ts";
import type { RID } from "@xpute/spec/rpc/rid.ts";
import type { Signature } from "@xpute/spec/net/crypto.ts";
import type { Handle, TransportEvent } from "@xpute/spec/net/xport/mod.ts";
import type { MeshAdvert, MeshDialSeed, MeshIceCandidateInit, MeshPlug, SignalFrame } from "@xpute/spec/net/xport/mesh.ts";
import { decode_dc_packet, encode_dc_packet } from "@xpute/spec/net/xport/mesh.ts";

const MAX_BUFFERED = 16 * 1024 * 1024;
const MAX_DC_MSG = 256 * 1024;

type NegotiationState =
  | { phase: "idle" }
  | { phase: "making_offer"; since_ms: number }
  | { phase: "setting_remote"; type: RTCSdpType; since_ms: number }
  | { phase: "collided"; polite_side: boolean };

type State = {
  seed: MeshDialSeed | null;
  ice_servers: RTCIceServer[];

  pc: RTCPeerConnection | null;
  dc: RTCDataChannel | null;
  sig_sock: Socket | null;
  sig_url: string | null;
  sig_room: string | null;
  sig_bound: 0 | 1;
  negotiation: NegotiationState;
  polite: boolean;
  sig_peer_handle: Handle | null;

  closed: 0 | 1;
  seed_epoch: u32;
};

export class RTCPlug implements MeshPlug {
  private hello_cb: ((handle: Handle, rid: RID, enc_hdr: Bytes, sig: Signature) => void) | null = null;
  private ev_cb: ((ev: TransportEvent) => void) | null = null;
  private packet_cb: ((handle: Handle, rid: RID, enc_hdr: Bytes, enc_body: Bytes | null, sig: Signature) => void) | null = null;

  private readonly st_by_handle = new Map<Handle, State>();

  attach(): void {
    // browser rtc plug builds per-handle runtime on seed(); keep as semantic hook
  }

  detach(): void {
    // best-effort shutdown of all handles
    for (const [handle] of this.st_by_handle) {
      // fire-and-forget; close_handle already best-effort
      void this.close_handle(handle);
    }
    this.st_by_handle.clear();
    this.hello_cb = null;
    this.ev_cb = null;
    this.packet_cb = null;
  }

  private ensure_open_handle(handle: Handle): State {
    const st = this.st_by_handle.get(handle);
    if (!st || st.closed) throw new NetworkFaultError(Errno.ENOENT, "mesh handle missing");
    return st;
  }

  private emit_event(ev: TransportEvent): void {
    this.ev_cb?.(ev);
  }

  private parse_rtc_rendezvous(rendezvous: string): { sig_url: string; room: string } {
    // accepted examples:
    //   rtc:https://api.example.com/rtc-sig?room=abc
    //   rtc:wss://api.example.com/rtc-sig?room=abc
    // room can also be passed in hash/path, but query `room` is canonical here.
    let u: URL;
    try {
      u = new URL(rendezvous);
    } catch {
      throw new NetworkFaultError(Errno.EINVAL, `invalid rtc rendezvous url: ${rendezvous}`);
    }

    const room = u.searchParams.get("room") ?? "";
    if (!room) throw new NetworkFaultError(Errno.EINVAL, `missing rtc rendezvous room: ${rendezvous}`);

    const sig_url = `${u.origin}${u.pathname}`;
    return { sig_url, room };
  }

  private ensure_pc(st: State, handle: Handle, epoch: u32): RTCPeerConnection {
    if (st.pc) return st.pc;

    const pc = new RTCPeerConnection({ iceServers: st.ice_servers });
    st.pc = pc;

    pc.onicecandidate = (ev) => {
      if (!st.sig_sock || !st.sig_room || st.closed || st.seed_epoch !== epoch) return;
      if (!ev.candidate) return;

      const json = ev.candidate.toJSON();
      if (!json.candidate) return;

      const msg: SignalFrame = {
        vec: Vector.RTC_SIGNAL_ICE,
        handle,
        room: st.sig_room,
        candidate: json as MeshIceCandidateInit,
      };
      st.sig_sock.emit("rtc:signal", msg);
    };

    pc.onconnectionstatechange = () => {
      if (st.closed || st.seed_epoch !== epoch) return;
      const s = pc.connectionState;
      if (s === "failed") this.emit_event({ vec: Vector.PEER_UNSTABLE, handle, reason: "rtc_failed" });
      else if (s === "disconnected") this.emit_event({ vec: Vector.LINK_DOWN, handle, reason: "rtc_disconnected" });
      else if (s === "connected") this.emit_event({ vec: Vector.LINK_UP, handle });
      else if (s === "closed") this.emit_event({ vec: Vector.LINK_DOWN, handle, reason: "rtc_closed" });
    };

    pc.oniceconnectionstatechange = () => {
      if (st.closed || st.seed_epoch !== epoch) return;
      const ice = pc.iceConnectionState;
      if (ice === "failed") {
        this.emit_event({ vec: Vector.PEER_UNSTABLE, handle, reason: "ice_failed" });
      } else if (ice === "disconnected") {
        this.emit_event({ vec: Vector.LINK_DOWN, handle, reason: "ice_disconnected" });
      }
    };

    pc.onicegatheringstatechange = () => {
      // gathering 완료 시점만 추적 (optional logging/telemetry)
      if (st.closed || st.seed_epoch !== epoch) return;
      // noop for now: gathering state transitions don't emit transport events
    };

    pc.ondatachannel = (ev) => {
      this.bind_dc(st, handle, ev.channel, epoch);
    };

    pc.onnegotiationneeded = async () => {
      if (!st.sig_sock || !st.sig_room || st.closed || st.seed_epoch !== epoch) return;
      try {
        st.negotiation = { phase: "making_offer", since_ms: Date.now() };
        const offer = await pc.createOffer();
        if (st.closed || st.seed_epoch !== epoch) return;
        await pc.setLocalDescription(offer);
        if (!pc.localDescription) return;

        const msg: SignalFrame = {
          vec: pc.localDescription.type === "offer" ? Vector.RTC_SIGNAL_OFFER : Vector.RTC_SIGNAL_ANSWER,
          handle,
          room: st.sig_room,
          sdp: pc.localDescription.sdp ?? "",
        };
        st.sig_sock.emit("rtc:signal", msg);
      } catch {
        this.emit_event({ vec: Vector.PEER_UNSTABLE, handle, reason: "negotiation_failed" });
      } finally {
        st.negotiation = { phase: "idle" };
      }
    };

    return pc;
  }

  private bind_dc(st: State, handle: Handle, dc: RTCDataChannel, epoch: u32): void {
    if (st.dc && st.dc !== dc) {
      try {
        st.dc.close();
      } catch {}
    }
    st.dc = dc;
    dc.binaryType = "arraybuffer";

    dc.onopen = () => {
      if (st.closed || st.seed_epoch !== epoch) return;
      this.emit_event({ vec: Vector.LINK_UP, handle });
    };
    dc.onclose = () => {
      if (st.closed || st.seed_epoch !== epoch) return;
      this.emit_event({ vec: Vector.LINK_DOWN, handle, reason: "dc_closed" });
    };
    dc.onerror = () => {
      if (st.closed || st.seed_epoch !== epoch) return;
      this.emit_event({ vec: Vector.PEER_UNSTABLE, handle, reason: "dc_error" });
    };
    dc.onmessage = (ev) => {
      if (st.closed || st.seed_epoch !== epoch) return;

      // ArrayBuffer 처리
      if (!(ev.data instanceof ArrayBuffer)) {
        this.emit_event({ vec: Vector.PEER_UNSTABLE, handle, reason: "dc_msg_not_binary" });
        return;
      }

      const data = new Uint8Array(ev.data);

      if (data.byteLength > MAX_DC_MSG) {
        this.emit_event({ vec: Vector.PEER_UNSTABLE, handle, reason: "dc_msg_too_large" });
        return;
      }

      try {
        const pkt = decode_dc_packet(data);
        this.packet_cb?.(handle, pkt.rid, pkt.enc_hdr, pkt.enc_body, pkt.sig);
      } catch {
        this.emit_event({ vec: Vector.PEER_UNSTABLE, handle, reason: "dc_badmsg" });
      }
    };
  }

  private ensure_client_dc(st: State, handle: Handle, epoch: u32): void {
    const pc = this.ensure_pc(st, handle, epoch);
    if (st.dc) return;
    const dc = pc.createDataChannel("pxl-mesh", { ordered: true });
    this.bind_dc(st, handle, dc, epoch);
  }

  private ensure_sig_sock(st: State, handle: Handle, epoch: u32, sig_url: string, room: string): void {
    if (st.sig_sock && st.sig_url === sig_url && st.sig_room === room && st.sig_bound) return;

    if (st.sig_sock) {
      try {
        st.sig_sock.disconnect();
      } catch {}
      st.sig_sock = null;
      st.sig_bound = 0;
    }

    const sock = io(sig_url, {
      transports: ["websocket"],
      autoConnect: true,
      reconnection: true,
      auth: { mesh_handle: handle },
    });

    st.sig_sock = sock;
    st.sig_url = sig_url;
    st.sig_room = room;
    st.sig_bound = 0;
    st.polite = false;
    st.sig_peer_handle = null;

    sock.on("connect", () => {
      if (st.closed || st.seed_epoch !== epoch) return;
      const join: SignalFrame = {
        vec: Vector.RTC_SIGNAL_JOIN,
        handle,
        room,
        polite: 0,
      };
      sock.emit("rtc:signal", join);
      st.sig_bound = 1;
    });

    sock.on("disconnect", () => {
      if (st.closed || st.seed_epoch !== epoch) return;

      if (st.dc && st.dc.readyState === "open") {
        return;
      }

      this.emit_event({ vec: Vector.LINK_DOWN, handle, reason: "signal_disconnect" });
    });

    sock.on("rtc:signal", async (rx: SignalFrame) => {
      if (st.closed || st.seed_epoch !== epoch) return;
      if (rx.room !== room) return;
      if (rx.handle === handle) return;

      if (rx.vec === Vector.NET_HELLO) {
        const rid = rx.rid;
        if (!rid) return;
        const enc_hdr = encoding.b64url_to_bytes(rx.enc_hdr_b64url);
        const sig = encoding.b64url_to_bytes(rx.sig_b64url) as Signature;
        this.hello_cb?.(handle, rid, enc_hdr, sig);
        return;
      }

      if (rx.vec === Vector.RTC_SIGNAL_PEER_JOINED) {
        if (rx.handle === handle) return;

        if (st.sig_peer_handle) {
          if (st.sig_peer_handle === rx.handle) return;
          this.emit_event({ vec: Vector.PEER_UNSTABLE, handle, reason: "room_collision" });
          return;
        }

        st.sig_peer_handle = rx.handle;
        st.polite = handle > rx.handle;
        if (!st.polite) this.ensure_client_dc(st, handle, epoch);
        return;
      }

      if (rx.vec === Vector.RTC_SIGNAL_PEER_LEFT) {
        if (!st.sig_peer_handle) return;
        if (st.sig_peer_handle !== rx.handle) return;
        st.sig_peer_handle = null;

        if (st.dc) {
          try {
            st.dc.close();
          } catch {}
          st.dc = null;
        }
        if (st.pc) {
          try {
            st.pc.close();
          } catch {}
          st.pc = null;
        }
        st.negotiation = { phase: "idle" };

        this.emit_event({ vec: Vector.LINK_DOWN, handle, reason: "peer_left" });
        return;
      }

      // after peer lock: drop any foreign signaling noise
      if (st.sig_peer_handle && rx.handle !== st.sig_peer_handle) return;

      const pc = this.ensure_pc(st, handle, epoch);

      if (rx.vec === Vector.RTC_SIGNAL_OFFER || rx.vec === Vector.RTC_SIGNAL_ANSWER) {
        const desc: RTCSessionDescriptionInit = {
          type: rx.vec === Vector.RTC_SIGNAL_OFFER ? "offer" : "answer",
          sdp: rx.sdp,
        };
        if (desc.type !== "offer" && desc.type !== "answer") {
          this.emit_event({ vec: Vector.PEER_UNSTABLE, handle, reason: "sdp_type_unsupported" });
          return;
        }

        const ready_for_offer = st.negotiation.phase !== "making_offer" && pc.signalingState === "stable";
        const offer_collision = desc.type === "offer" && !ready_for_offer;
        const ignore_offer = offer_collision && !st.polite;
        if (ignore_offer) return;

        try {
          st.negotiation = {
            phase: "setting_remote",
            type: desc.type,
            since_ms: Date.now(),
          };
          await pc.setRemoteDescription(desc);
          st.negotiation = { phase: "idle" };

          if (desc.type === "offer") {
            await pc.setLocalDescription(await pc.createAnswer());
            if (!pc.localDescription || !st.sig_sock || !st.sig_room) return;

            const tx: SignalFrame = {
              vec: Vector.RTC_SIGNAL_ANSWER,
              handle,
              room: st.sig_room,
              sdp: pc.localDescription.sdp ?? "",
            };
            st.sig_sock.emit("rtc:signal", tx);
          }
        } catch {
          this.emit_event({ vec: Vector.PEER_UNSTABLE, handle, reason: "sdp_apply_failed" });
        } finally {
          st.negotiation = { phase: "idle" };
        }
        return;
      }

      if (rx.vec === Vector.RTC_SIGNAL_ICE) {
        try {
          await pc.addIceCandidate(rx.candidate);
        } catch {
          this.emit_event({ vec: Vector.PEER_UNSTABLE, handle, reason: "ice_add_failed" });
        }
        return;
      }
    });
  }

  apply_advert(handle: Handle, advert?: MeshAdvert): void {
    const st = this.st_by_handle.get(handle);
    if (!st || st.closed) return;

    const prev = st.ice_servers;
    const next_ice = (() => {
      const ice = advert?.ice_servers;
      if (!(ice && ice.length)) return [];
      return ice.map((x) => {
        const cred = x.credential;
        if (cred === undefined || cred === null) return x;
        return { ...x, credential: String(cred) };
      }) as RTCIceServer[];
    })();

    // content-equality (shallow) to suppress no-op churn
    let same = prev.length === next_ice.length;
    for (let i = 0; same && i < prev.length; i++) {
      const a = prev[i];
      const b = next_ice[i];

      if ((a.username ?? "") !== (b.username ?? "")) same = false;
      if (String(a.credential ?? "") !== String(b.credential ?? "")) same = false;

      const au = Array.isArray(a.urls) ? a.urls : [a.urls];
      const bu = Array.isArray(b.urls) ? b.urls : [b.urls];
      if (au.length !== bu.length) same = false;
      for (let j = 0; same && j < au.length; j++) {
        if (au[j] !== bu[j]) same = false;
      }
    }
    if (same) return;

    st.ice_servers = next_ice;

    // If pc already exists, try live reconfigure; otherwise force rebuild on next use.
    if (!st.pc) return;
    try {
      st.pc.setConfiguration({ iceServers: st.ice_servers });
      return;
    } catch {}
    try {
      st.pc.close();
    } catch {}
    st.pc = null;
    if (st.dc) {
      try {
        st.dc.close();
      } catch {}
      st.dc = null;
    }
    st.negotiation = { phase: "idle" };
  }

  private st_of(handle: Handle): State {
    let st = this.st_by_handle.get(handle);
    if (st) return st;

    st = {
      seed: null,
      ice_servers: [],
      pc: null,
      dc: null,
      sig_sock: null,
      sig_url: null,
      sig_room: null,
      sig_bound: 0,
      negotiation: { phase: "idle" },
      polite: false,
      sig_peer_handle: null,
      closed: 0,
      seed_epoch: 0,
    };
    this.st_by_handle.set(handle, st);
    return st;
  }

  async seed(handle: Handle, dial: MeshDialSeed): Promise<void> {
    const st = this.st_of(handle);
    st.seed = dial;
    st.closed = 0;
    st.seed_epoch = U32(st.seed_epoch + 1);

    const epoch = st.seed_epoch;

    // reset prior runtime objects on reseed
    if (st.dc) {
      try {
        st.dc.close();
      } catch {}
      st.dc = null;
    }
    if (st.pc) {
      try {
        st.pc.close();
      } catch {}
      st.pc = null;
    }
    if (st.sig_sock) {
      try {
        st.sig_sock.disconnect();
      } catch {}
      st.sig_sock = null;
    }
    st.sig_bound = 0;
    st.sig_url = null;
    st.sig_room = null;
    st.negotiation = { phase: "idle" };

    if (dial.scheme === "rtc") {
      const { sig_url, room } = this.parse_rtc_rendezvous(dial.rendezvous);
      this.ensure_sig_sock(st, handle, epoch, sig_url, room);
      return;
    }

    // turn-only seed: cache only (signaling may come later via advert upgrade in transport)
    // NOTE:
    // TURN is not applied here as iceServers because transport currently seeds only one scheme at a time.
    // If transport passes rtc+turn bundle later, map turn seed into ice_servers at seed() time.
  }

  async send_hello_bootstrap(handle: Handle, rid: RID, enc_hdr: Bytes, sig: Signature): Promise<void> {
    const st = this.ensure_open_handle(handle);
    if (!st.sig_sock || !st.sig_room || !st.sig_bound) {
      throw new NetworkFaultError(Errno.ENOTCONN, "mesh signaling not connected");
    }
    const msg: SignalFrame = {
      vec: Vector.NET_HELLO,
      rid,
      handle,
      room: st.sig_room,
      enc_hdr_b64url: encoding.bytes_to_b64url(enc_hdr),
      sig_b64url: encoding.bytes_to_b64url(sig as unknown as Bytes),
    };
    st.sig_sock.emit("rtc:signal", msg);
  }

  async send_packet(handle: Handle, rid: RID, enc_hdr: Bytes, enc_body: Bytes | null, sig: Signature): Promise<void> {
    const st = this.ensure_open_handle(handle);
    if (!st.dc || st.dc.readyState !== "open") {
      throw new NetworkFaultError(Errno.ENOTCONN, "mesh datachannel not connected");
    }

    if (st.dc.bufferedAmount > MAX_BUFFERED) {
      throw new NetworkFaultError(Errno.EAGAIN, "datachannel buffer full");
    }

    try {
      const msg = encode_dc_packet(rid, enc_hdr, enc_body, sig);
      st.dc.send(msg.buffer); // ArrayBuffer 전송

      if (st.dc.bufferedAmount > MAX_BUFFERED / 2) {
        this.emit_event({ vec: Vector.PEER_UNSTABLE, handle, reason: "datachannel_buffering" });
      }
    } catch {
      throw new NetworkFaultError(Errno.EIO, "mesh datachannel send failed");
    }
  }

  async restart_ice(handle: Handle): Promise<void> {
    const st = this.ensure_open_handle(handle);
    if (!st.pc) throw new NetworkFaultError(Errno.ENOENT, "no peer connection");

    try {
      const offer = await st.pc.createOffer({ iceRestart: true });
      await st.pc.setLocalDescription(offer);

      if (!st.sig_sock || !st.sig_room || !st.pc.localDescription) return;

      const msg: SignalFrame = {
        vec: Vector.RTC_SIGNAL_OFFER,
        handle,
        room: st.sig_room,
        sdp: st.pc.localDescription.sdp ?? "",
      };
      st.sig_sock.emit("rtc:signal", msg);
    } catch {
      this.emit_event({ vec: Vector.PEER_UNSTABLE, handle, reason: "ice_restart_failed" });
    }
  }

  on_hello_bootstrap(cb: (handle: Handle, rid: RID, enc_hdr: Bytes, sig: Signature) => void): void {
    this.hello_cb = cb;
  }

  on_event(cb: (ev: TransportEvent) => void): void {
    this.ev_cb = cb;
  }

  on_packet(cb: (handle: Handle, rid: RID, enc_hdr: Bytes, enc_body: Bytes | null, sig: Signature) => void): void {
    this.packet_cb = cb;
  }

  async close_handle(handle: Handle): Promise<void> {
    const st = this.st_by_handle.get(handle);
    if (!st) return;

    st.closed = 1;

    if (st.dc) {
      try {
        st.dc.close();
      } catch {}
      st.dc = null;
    }
    if (st.pc) {
      try {
        st.pc.close();
      } catch {}
      st.pc = null;
    }
    if (st.sig_sock) {
      try {
        st.sig_sock.disconnect();
      } catch {}
      st.sig_sock = null;
    }
    st.sig_bound = 0;
    st.sig_url = null;
    st.sig_room = null;

    // No LINK_DOWN emit here by default:
    // transport-side teardown may be cleanup after terminal decision, and duplicate
    // edge hints are not useful unless we track actual RTC link state.

    this.st_by_handle.delete(handle);
  }

  inject_link_up(handle: Handle): void {
    this.emit_event({ vec: Vector.LINK_UP, handle });
  }

  inject_link_down(handle: Handle, reason?: string): void {
    this.emit_event({ vec: Vector.LINK_DOWN, handle, reason });
  }

  inject_peer_unstable(handle: Handle, reason?: string): void {
    this.emit_event({ vec: Vector.PEER_UNSTABLE, handle, reason });
  }

  debug_seed(handle: Handle): MeshDialSeed | null {
    const st = this.st_by_handle.get(handle);
    if (!st || st.closed) return null;
    return st.seed;
  }

  // ---- dev/test injection helpers (non-interface) ----

  inject_hello_bootstrap(handle: Handle, rid: RID, enc_hdr: Bytes, sig: Signature): void {
    this.hello_cb?.(handle, rid, enc_hdr, sig);
  }

  inject_packet(handle: Handle, rid: RID, enc_hdr: Bytes, enc_body: Bytes | null, sig: Signature): void {
    this.packet_cb?.(handle, rid, enc_hdr, enc_body, sig);
  }

  inject_event(ev: TransportEvent): void {
    this.ev_cb?.(ev);
  }
}
