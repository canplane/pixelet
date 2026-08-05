// @xpute/core/kit/encoding.ts

/**
 * Encoding utilities
 * - UTF-8
 * - Base64url (URL-safe, no padding)
 * - UUID v4
 * - Hex
 */

import type { Bytes } from "@xpute/core/kit/array.ts";

// ============ UTF-8 ============

export const te = new TextEncoder();
export const td = new TextDecoder();

// export function utf8_encode(s: string): Bytes {
//   return te.encode(s) as Bytes;
// }
// export function utf8_decode(b: ArrayBuffer | ArrayBufferView): string {
//   const buf = "buffer" in b ? b.buffer : b;
//   const off = "byteOffset" in b ? b.byteOffset : 0;
//   const len = b.byteLength;
//   return td.decode(new Uint8Array(buf, off, len));
// }

// ============ Base64url ============

export type Base64URL = string; // 22 chars
type Base64 = string;

export function is_b64url(s: string): boolean {
  // base64url charset only (no '=')
  // NOTE: keep ASCII-only
  for (let i = 0; i < s.length; i++) {
    const c = s.charCodeAt(i);
    const ok = (c >= 0x30 && c <= 0x39) || // 0-9
      (c >= 0x41 && c <= 0x5a) || // A-Z
      (c >= 0x61 && c <= 0x7a) || // a-z
      c === 0x2d || // -
      c === 0x5f; // _
    if (!ok) return false;
  }
  return true;
}

export function ct_eq(a: Bytes, b: Bytes): boolean {
  if (a.byteLength !== b.byteLength) return false;
  let v = 0;
  for (let i = 0; i < a.byteLength; i++) v |= a[i] ^ b[i];
  return v === 0;
}

/**
 * Convert bytes to base64url
 * No padding, URL-safe
 */
export function bytes_to_b64url(bytes: Bytes): Base64URL {
  // Uint8Array -> binary string (chunked; avoids Array.from/join blowup)
  // NOTE: btoa expects Latin1 string where each charCode is 0..255.
  let bin_str = "";
  const n = bytes.byteLength;
  const CHUNK = 0x8000; // 32KB-ish; safe for apply arg limits
  for (let i = 0; i < n; i += CHUNK) {
    const end = Math.min(i + CHUNK, n);
    const sub = bytes.subarray(i, end);
    // eslint-disable-next-line @typescript-eslint/no-unsafe-argument
    bin_str += String.fromCharCode.apply(null, sub as unknown as number[]);
  }

  const b64: Base64 = btoa(bin_str);

  // base64 → base64url
  return b64.replace(/\+/g, "-").replace(/\//g, "_").replace(/=/g, "");
}

/**
 * Convert base64url to bytes
 */
export function b64url_to_bytes(b64url: Base64URL): Bytes {
  // base64url → base64
  let b64: Base64 = b64url.replace(/-/g, "+").replace(/_/g, "/");

  // padding 복원
  while (b64.length % 4) {
    b64 += "=";
  }

  const bin_str: string = atob(b64);
  const n = bin_str.length;
  const out = new Uint8Array(n);
  for (let i = 0; i < n; i++) out[i] = bin_str.charCodeAt(i) & 0xff;
  return out;
}

export function is_pid_b64url_16(pid: unknown, pid_bin: Bytes): boolean {
  // strict: pid must be a base64url string encoding exactly 16 bytes, no padding.
  if (typeof pid !== "string") return false;
  if (pid.length !== 22) return false; // 16 bytes => 22 chars (unpadded base64url)
  if (!is_b64url(pid)) return false;
  try {
    const b = b64url_to_bytes(pid);
    return b.byteLength === 16 && ct_eq(b, pid_bin);
  } catch {
    return false;
  }
}

// ============ UUID v4 ============

export type UUID = Bytes; // 16 bytes

/**
 * Generate UUID v4 (random)
 */
export function generate_uuid(): UUID {
  const bytes: UUID = new Uint8Array(16);
  crypto.getRandomValues(bytes);

  bytes[6] = (bytes[6] & 0x0f) | 0x40; // version 4
  bytes[8] = (bytes[8] & 0x3f) | 0x80; // variant 2

  return bytes;
}

/**
 * UUID to base64url (22 chars)
 */
export function uuid_to_b64url(uuid: UUID): Base64URL {
  if (uuid.length !== 16) {
    throw new Error("UUID: Invalid length");
  }
  return bytes_to_b64url(uuid);
}

/**
 * Base64url to UUID
 */
export function b64url_to_uuid(b64url: Base64URL): UUID {
  const bytes: UUID = b64url_to_bytes(b64url);
  if (bytes.length !== 16) {
    throw new Error("UUID: Invalid format");
  }
  return bytes;
}

/**
 * Compare UUIDs
 */
export function uuid_equals(a: UUID, b: UUID): boolean {
  if (a.length !== 16 || b.length !== 16) return false;

  for (let i = 0; i < 16; i++) {
    if (a[i] !== b[i]) return false;
  }

  return true;
}

// ============ Hex ============

/**
 * Bytes to hex string
 */
export function bytes_to_hex(bytes: Bytes): string {
  return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}

/**
 * Hex string to bytes
 */
export function hex_to_bytes(hex: string): Bytes {
  if (hex.length % 2 !== 0) {
    throw new Error("Hex: Invalid length");
  }

  const bytes = new Uint8Array(hex.length / 2);
  for (let i = 0; i < hex.length; i += 2) {
    bytes[i / 2] = parseInt(hex.slice(i, i + 2), 16);
  }
  return bytes;
}
