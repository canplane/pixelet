// @xpute/kit/codec/hash.ts

import type { u64 } from "../abi/word.ts";
import type { Bytes } from "../abi/array.ts";
import { bytes_to_hex, te } from "./encoding.ts";

const FNV64_OFFSET = 0xcbf29ce484222325n;
const FNV64_PRIME = 0x100000001b3n;

/** FNV-1a 64 over UTF-8, the same word the Rust side and the spec generator compute. */
export function fnv1a64_str(s: string): u64 {
  return fnv1a64_bytes(te.encode(s));
}

export function fnv1a64_bytes(bytes: Bytes): u64 {
  let h = FNV64_OFFSET;
  for (let i = 0; i < bytes.length; i++) {
    h ^= BigInt(bytes[i]);
    h = BigInt.asUintN(64, h * FNV64_PRIME);
  }
  return h;
}

export async function sha256(bytes: Bytes): Promise<Bytes> {
  return new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
}

export function sha256_from_str(s: string): Promise<Bytes> {
  return sha256(te.encode(s));
}

export async function sha256_hex(s: string): Promise<string> {
  return bytes_to_hex(await sha256_from_str(s));
}
