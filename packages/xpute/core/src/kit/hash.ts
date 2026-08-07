// @xpute/core/kit/hash.ts

import { type u16, U32, type u32, U8, type u8 } from "./word.ts";
import type { Bytes } from "./array.ts";
import { bytes_to_hex, te } from "./encoding.ts";

// ============ FNV-1a 64-bit (8 bytes output) ============

type FNVState = { h1: u32; h2: u32; p: u32 };

const FNV_OFFSET_32: u32 = 0x811c9dc5;
const FNV_PRIME_32: u32 = 0x01000193;

export function fnv1a_64_init(): FNVState {
  return { h1: FNV_OFFSET_32, h2: FNV_OFFSET_32, p: 0 };
}

export function fnv1a_64_u16le(st: FNVState, x: u16): void {
  const b0: u8 = U8(x);
  const b1: u8 = U8(x >> 8);
  fnv1a_64_byte(st, b0);
  fnv1a_64_byte(st, b1);
}

export function fnv1a_64_bytes(st: FNVState, bytes: Bytes): void {
  for (let i = 0; i < bytes.length; i++) fnv1a_64_byte(st, bytes[i]);
}

function fnv1a_64_byte(st: FNVState, b: u8): void {
  const bx: u8 = U8(b);

  if ((st.p++ & 1) === 0) {
    st.h1 ^= bx;
    st.h1 = U32(Math.imul(st.h1, FNV_PRIME_32));
  } else {
    st.h2 ^= bx;
    st.h2 = U32(Math.imul(st.h2, FNV_PRIME_32));
  }
}

export function fnv1a_64_final(st: FNVState): Bytes {
  const out: Bytes = new Uint8Array(8);
  const view = new DataView(out.buffer, out.byteOffset, out.byteLength);
  view.setUint32(0, st.h1, true);
  view.setUint32(4, st.h2, true);
  return out;
}

export function fnv1a_64_hex(bytes: Bytes): string {
  const st = fnv1a_64_init();
  fnv1a_64_bytes(st, bytes);
  return bytes_to_hex(fnv1a_64_final(st));
}

// ============ SHA-256 ============
// Converts string data to a fixed-length hash via SHA-256.
// Mainly used to derive cache keys for large query strings.

export async function sha256(bytes: Bytes): Promise<Bytes> {
  return new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
}

export function sha256_from_str(s: string): Promise<Bytes> {
  return sha256(te.encode(s));
}

export async function sha256_hex(s: string): Promise<string> {
  return bytes_to_hex(await sha256_from_str(s));
}
