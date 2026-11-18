// @xpute/kit/codec/encoding.ts

/**
 * Encoding utilities
 * - UTF-8
 * - Base64url (URL-safe, no padding)
 * - UUID v4
 * - Hex
 */

import type { ByteArray } from "@xpute/kit/abi/array.ts";
import { ERRNO_EINVAL } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";

// ============ UTF-8 ============

export const te = new TextEncoder();
export const td = new TextDecoder();

// ============ Base64url ============

export type Base64Url = string;
type Base64 = string;

export function is_b64url(s: string): boolean {
  // base64url charset only (no '=')
  // NOTE: keep ASCII-only
  for (let i = 0; i < s.length; i++) {
    const c = s.charCodeAt(i);
    const ok =
      (c >= 0x30 && c <= 0x39) || // 0-9
      (c >= 0x41 && c <= 0x5a) || // A-Z
      (c >= 0x61 && c <= 0x7a) || // a-z
      c === 0x2d || // -
      c === 0x5f; // _
    if (!ok) return false;
  }
  return true;
}

export function ct_eq(a: ByteArray, b: ByteArray): boolean {
  if (a.byteLength !== b.byteLength) return false;
  let v = 0;
  for (let i = 0; i < a.byteLength; i++) v |= a[i] ^ b[i];
  return v === 0;
}

/**
 * Convert bytes to base64url
 * No padding, URL-safe
 */
export function bytes_to_b64url(bytes: ByteArray): Base64Url {
  // Uint8Array -> binary string (chunked; avoids Array.from/join blowup)
  // NOTE: btoa expects Latin1 string where each charCode is 0..255.
  let bin_str = "";
  const n = bytes.byteLength;
  const CHUNK = 0x8000; // 32KB-ish; safe for apply arg limits
  for (let i = 0; i < n; i += CHUNK) {
    const end = Math.min(i + CHUNK, n);
    const sub = bytes.subarray(i, end);
    bin_str += String.fromCharCode.apply(null, sub as unknown as number[]);
  }

  const b64: Base64 = btoa(bin_str);

  // base64 → base64url
  return b64.replace(/\+/g, "-").replace(/\//g, "_").replace(/=/g, "");
}

/**
 * Convert base64url to bytes
 */
export function b64url_to_bytes(b64url: Base64Url): ByteArray {
  // base64url → base64
  let b64: Base64 = b64url.replace(/-/g, "+").replace(/_/g, "/");

  // The padding is put back.
  while (b64.length % 4) {
    b64 += "=";
  }

  const bin_str: string = atob(b64);
  const n = bin_str.length;
  const out = new Uint8Array(n);
  for (let i = 0; i < n; i++) out[i] = bin_str.charCodeAt(i) & 0xff;
  return out;
}

export function is_pid_b64url_16(pid: unknown, pid_bin: ByteArray): boolean {
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

export type Uuid = ByteArray; // 16 bytes

/**
 * Generate UUID v4 (random)
 */
export function generate_uuid(): Uuid {
  const bytes: Uuid = new Uint8Array(16);
  crypto.getRandomValues(bytes);

  bytes[6] = (bytes[6] & 0x0f) | 0x40; // Marks the UUID as version 4.
  bytes[8] = (bytes[8] & 0x3f) | 0x80; // Marks it as RFC 4122's variant.

  return bytes;
}

/**
 * UUID to base64url (22 chars)
 */
export function uuid_to_b64url(uuid: Uuid): Base64Url {
  if (uuid.length !== 16) {
    throw new XputeError(ERRNO_EINVAL);
  }
  return bytes_to_b64url(uuid);
}

/**
 * Base64url to UUID
 */
export function b64url_to_uuid(b64url: Base64Url): Uuid {
  const bytes: Uuid = b64url_to_bytes(b64url);
  if (bytes.length !== 16) {
    throw new XputeError(ERRNO_EINVAL);
  }
  return bytes;
}

/**
 * Compare UUIDs
 */
export function uuid_equals(a: Uuid, b: Uuid): boolean {
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
export function bytes_to_hex(bytes: ByteArray): string {
  return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}

/**
 * Hex string to bytes
 */
export function hex_to_bytes(hex: string): ByteArray {
  if (hex.length % 2 !== 0) {
    throw new XputeError(ERRNO_EINVAL);
  }

  const bytes = new Uint8Array(hex.length / 2);
  for (let i = 0; i < hex.length; i += 2) {
    // `parseInt` reads what it can and stops, so it would take "1g" as 1 and
    // store "zz", its NaN, as 0; each pair is checked first.
    const pair = hex.slice(i, i + 2);
    if (!/^[0-9a-fA-F]{2}$/.test(pair)) throw new XputeError(ERRNO_EINVAL);
    bytes[i / 2] = parseInt(pair, 16);
  }
  return bytes;
}
