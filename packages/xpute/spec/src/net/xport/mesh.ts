// @xpute/spec/net/xport/mesh.ts - MESH transport contracts

import { type Bytes, is_bytes } from "@xpute/spec/lib/kit/array.ts";
import { TLVReader, TLVWriter } from "@xpute/spec/io/marshal/tlv.ts";

import { Vector } from "@xpute/spec/control/vec.ts";
import type { RID } from "@xpute/spec/rpc/rid.ts";
import type { Signature } from "../crypto.ts";
import type { Dial, Handle, TransportEvent } from "./mod.ts";

/**
 * Mesh ICE server config (runtime-agnostic; JSON-serializable)
 * - mirrors RTCIceServer shape (subset)
 */
export type MeshIceServer = {
  urls: string | readonly string[];
  username?: string;
  credential?: string;
};

/**
 * Mesh ICE candidate init (runtime-agnostic; JSON-serializable)
 * - mirrors RTCIceCandidateInit shape (subset)
 */
export type MeshIceCandidateInit = {
  candidate: string;
  sdpMid?: string | null;
  sdpMLineIndex?: number | null;
  usernameFragment?: string | null;
};

/**
 * Mesh advert: peer-provided bootstrap/signaling hints
 *
 * Fields:
 * - rendezvous_dial: fallback bootstrap hint
 * - signal_dial: preferred signaling path for HELLO bootstrap
 */
export type MeshAdvert = {
  rendezvous_dial?: Dial;
  signal_dial?: Dial;
  ice_servers?: readonly MeshIceServer[];
};

/**
 * Mesh dial seed: bootstrap config consumed by plug
 *
 * Policy:
 * - 'rtc': WebRTC signaling endpoint (includes room)
 * - 'turn': TURN/ICE server config (no signaling)
 */
export type MeshDialSeed = { scheme: "rtc"; rendezvous: string } | { scheme: "turn"; url: string };

/**
 * MeshPlug: Common interface for WebRTC endpoints and signaling relays
 *
 * Browser: Full WebRTC + DataChannel implementation
 * Server: Signaling relay only (send_packet throws ENOTSUP)
 */
export interface MeshPlug {
  seed(handle: Handle, dial: MeshDialSeed): Promise<void>;
  send_hello_bootstrap(handle: Handle, rid: RID, enc_hdr: Bytes, sig: Signature): Promise<void>;
  send_packet(handle: Handle, rid: RID, enc_hdr: Bytes, enc_body: Bytes | null, sig: Signature): Promise<void>;
  apply_advert(handle: Handle, advert?: MeshAdvert): void;
  on_hello_bootstrap(cb: (handle: Handle, rid: RID, enc_hdr: Bytes, sig: Signature) => void): void;
  on_event(cb: (ev: TransportEvent) => void): void;
  on_packet(cb: (handle: Handle, rid: RID, enc_hdr: Bytes, enc_body: Bytes | null, sig: Signature) => void): void;
  close_handle(handle: Handle): Promise<void>;
}

// ============ Signaling wire protocol ============

export type SignalFrame =
  // Client → Server
  | { vec: Vector.RTC_SIGNAL_JOIN; handle: Handle; room: string; polite: 0 | 1 }
  // Server → Client
  | { vec: Vector.RTC_SIGNAL_PEER_JOINED; handle: Handle; room: string; polite: 0 | 1 }
  | { vec: Vector.RTC_SIGNAL_PEER_LEFT; handle: Handle; room: string }
  // Bidirectional (hello uses NET_HELLO)
  | { vec: Vector.NET_HELLO; rid: RID; handle: Handle; room: string; enc_hdr_b64url: string; sig_b64url: string }
  | { vec: Vector.RTC_SIGNAL_OFFER; handle: Handle; room: string; sdp: string }
  | { vec: Vector.RTC_SIGNAL_ANSWER; handle: Handle; room: string; sdp: string }
  | { vec: Vector.RTC_SIGNAL_ICE; handle: Handle; room: string; candidate: MeshIceCandidateInit };

// ============ DataChannel packet encoding (BINARY) ============

export function encode_dc_packet(rid: RID, enc_hdr: Bytes, enc_body: Bytes | null, sig: Signature): Bytes {
  const w = new TLVWriter();
  w.str(rid as string);
  w.bytes(enc_hdr);
  w.bytes(enc_body ?? new Uint8Array(0));
  w.bytes(sig as unknown as Bytes);
  return w.finish();
}

export function decode_dc_packet(data: Bytes): {
  rid: RID;
  enc_hdr: Bytes;
  enc_body: Bytes | null;
  sig: Signature;
} {
  const r = new TLVReader(data);
  const it = r.read_all();

  if (it.length !== 4) throw new Error("decode_dc_packet: bad packet");

  const rid = it[0];
  const enc_hdr = it[1];
  const enc_body_raw = it[2];
  const sig = it[3];

  if (typeof rid !== "string") throw new Error("decode_dc_packet: bad rid");
  if (!is_bytes(enc_hdr)) throw new Error("decode_dc_packet: bad enc_hdr");
  if (!is_bytes(enc_body_raw)) throw new Error("decode_dc_packet: bad enc_body");
  if (!is_bytes(sig)) throw new Error("decode_dc_packet: bad sig");

  return {
    rid: rid as RID,
    enc_hdr,
    enc_body: enc_body_raw.byteLength ? enc_body_raw : null,
    sig: sig as Signature,
  };
}

/**
 * Parse mesh dial into seed config
 *
 * Supported formats:
 * - rtc:<rendezvous>
 * - turn:<url>
 */
export function split_dial(dial: Dial): MeshDialSeed {
  const s = String(dial);

  if (s.startsWith("rtc:")) {
    const rendezvous = s.slice("rtc:".length);
    if (!rendezvous.length) throw new Error(`split_dial: empty rtc rendezvous: ${s}`);
    return { scheme: "rtc", rendezvous };
  }

  if (s.startsWith("turn:")) {
    const url = s.slice("turn:".length);
    if (!url.length) throw new Error(`split_dial: empty turn: ${s}`);
    return { scheme: "turn", url: `turn://${url}` };
  }

  throw new Error(`split_dial: unsupported dial: ${s}`);
}
