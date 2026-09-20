// @xpute/core/codec/hash.ts

import type { u64 } from "../abi/word.ts";
import type { Bytes } from "../abi/array.ts";
import { bytes_to_hex, te } from "./encoding.ts";

// ============ FNV-1a 64-bit, as one word ============

const FNV64_OFFSET = 0xcbf29ce484222325n;
const FNV64_PRIME = 0x100000001b3n;

/** FNV-1a over a string's UTF-8 bytes, 64 bits wide — the same word the
 * Rust side and script/spec.ts compute, so an id hashed anywhere is one
 * number. */
export function fnv1a64_str(s: string): u64 {
  return fnv1a64_bytes(te.encode(s));
}

/** The same word over a byte string, for an id built from numbers rather
 * than spelled out as text. */
export function fnv1a64_bytes(bytes: Bytes): u64 {
  let h = FNV64_OFFSET;
  for (let i = 0; i < bytes.length; i++) {
    h ^= BigInt(bytes[i]);
    h = BigInt.asUintN(64, h * FNV64_PRIME);
  }
  return h;
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
