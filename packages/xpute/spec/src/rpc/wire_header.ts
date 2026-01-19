// @xpute/spec/rpc/wire_header.ts

import { type i32, U16, type u16, type u32 } from "../lib/kit/word.ts";
import { type Bytes, is_bytes } from "@xpute/spec/lib/kit/array.ts";
import { TLVReader, TLVWriter } from "@xpute/spec/io/marshal/tlv.ts";

import type { SignalFrame } from "@xpute/spec/ipc/frame.ts";
import { FrameKind } from "@xpute/spec/ipc/frame.ts";
import type { NodeSelector } from "@xpute/spec/rpc/node.ts";
import type { RID } from "@xpute/spec/rpc/rid.ts";

// -----------------------------------------------------------------------------
// Wire header: encrypt(pack(tx_id, errno, vec, flags, args))
// Body is transported separately (transport decides mux/demux).
// -----------------------------------------------------------------------------

export type WireHeader = Bytes;

// minimal hard caps (DoS guard); tune later.
const MAX_ARGS = 64 * 1024; // 64 KiB

export function pack_wire_header(frame: SignalFrame): WireHeader {
  const args: Bytes = frame.args;
  const args_len: u32 = args.length;

  if (args_len > MAX_ARGS) throw new Error(`WireHeader: args too big: ${args_len}`);

  const w = new TLVWriter();
  w.u32(frame.tx_id);
  w.i32(frame.errno);
  w.u16(U16(frame.vec));
  w.u32(frame.flags);
  w.bytes(args);
  return w.finish();
}

export function unpack_wire_header(hdr: WireHeader, sel: NodeSelector, rid: RID): SignalFrame | null {
  try {
    const r = new TLVReader(hdr);
    const it = r.read_all();
    if (it.length !== 5) return null; // strict: exactly 5 fields

    const tx_id = it[0];
    const errno = it[1];
    const vec = it[2];
    const flags = it[3];
    const args = it[4];

    if (typeof tx_id !== "number") return null;
    if (typeof errno !== "number") return null;
    if (typeof vec !== "number") return null;
    if (typeof flags !== "number") return null;
    if (!is_bytes(args)) return null;

    const args_len: u32 = args.byteLength;
    if (args_len > MAX_ARGS) return null;

    return {
      kind: FrameKind.SIGNAL,
      sel,
      rid,
      opts: {},

      tx_id: tx_id as u32,
      errno: errno as i32,
      vec: vec as u16,
      flags: flags as u32,
      args,
      body: null,
    };
  } catch {
    return null;
  }
}

export function sniff_wire_vec(hdr: WireHeader): number | null {
  try {
    const r = new TLVReader(hdr);
    let idx = 0;
    for (const v of r) {
      if (idx === 2) return typeof v === "number" ? v : null;
      if (++idx > 2) break;
    }
    return null;
  } catch {
    return null;
  }
}

/**
 * @unused 참고용
 * [System OpCodes]
 * vector가 0(System)일 때 사용되는 내부 명령 코드
 * (Handshake, Keep-alive 등 시스템 레벨 통신용)
 */
export const enum NetSysOp {
  HELLO = 0x01, // 접속 시 최초 핸드쉐이크
  PING = 0x02, // Heartbeat
  PONG = 0x03,
  ERROR = 0xff,
}
