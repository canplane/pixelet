// @xpute/spec/net/crypto.ts

/**
 * Cryptographic primitives (v1)
 *
 * Goals:
 * - Identity: pid + (ed25519 signing) + (x25519 dh)
 * - E2E: header/body split encryption (2 AEAD calls)
 * - 1 signature per packet: sign(rid || enc_hdr || body_len || body_hash?)
 *
 * Stack:
 * - Sign:   Ed25519
 * - DH:     X25519
 * - KDF:    HKDF-SHA256 (salt=rid_bin, info="xpute/v1/hdr" or "xpute/v1/body")
 * - AEAD:   AES-GCM (iv=12 bytes, out = iv || ciphertext)
 */

import * as ed25519 from "@noble/ed25519";
import { x25519 } from "@noble/curves/ed25519.js";
import { sha512 } from "@noble/hashes/sha2.js";

import type { u32 } from "../lib/kit/word.ts";
import { as_bytes, type Bytes } from "@xpute/spec/lib/kit/array.ts";
import type { Base64URL, UUID } from "@xpute/spec/lib/kit/encoding.ts";
import * as encoding from "@xpute/spec/lib/kit/encoding.ts";
import type { Stream } from "@xpute/spec/io/scalar.ts";
import * as scalar from "@xpute/spec/io/scalar.ts";

import type { RID } from "@xpute/spec/rpc/rid.ts";

ed25519.hashes.sha512 = sha512;

// ============ Types ============

export type PID = Base64URL; // 22 chars (b64url 16 bytes)

export type Signature = Bytes; // 64 bytes

export type IdentityKey = Bytes; // 32 bytes (ed25519, x25519)

/**
 * Identity: Local cryptographic identity (my keys)
 * - Generated once per node
 * - Used for signing/encryption
 * - pid derived from public keys
 */
export type Identity = {
  pid: PID;

  sign_pub: IdentityKey;
  sign_priv: IdentityKey;

  dh_pub: IdentityKey;
  dh_priv: IdentityKey;
};

// ============ Const / Helpers ============

const IV_SZ: u32 = 12;

const INFO_HDR: Bytes = encoding.te.encode("xpute/v1/hdr");
const INFO_BODY: Bytes = encoding.te.encode("xpute/v1/body");
const INFO_BOOT: Bytes = encoding.te.encode("xpute/v1/bootstrap");

const need_subtle = (): SubtleCrypto => {
  const s: SubtleCrypto = globalThis.crypto?.subtle;
  if (!s) throw new Error("crypto.subtle unavailable");
  return s;
};

// ============ Random ============

/**
 * Generate cryptographically secure random bytes.
 *
 * SECURITY:
 * - Uses crypto.getRandomValues (CSPRNG)
 * - Throws if unavailable (no weak fallback)
 * - Always returns ArrayBuffer-backed Bytes
 *
 * @throws Error if crypto.getRandomValues unavailable
 */
export const random_bytes = (n: number): Bytes => {
  const grv = globalThis.crypto?.getRandomValues;
  if (!grv) throw new Error("crypto.getRandomValues unavailable");
  const out = new Uint8Array(n);
  grv.call(globalThis.crypto, out);
  return as_bytes(out);
};

// ============ Hash ============

export async function hash256(x: Bytes): Promise<Bytes> {
  const subtle: SubtleCrypto = need_subtle();
  const out: ArrayBuffer = await subtle.digest("SHA-256", x);
  return new Uint8Array(out);
}

// ============ HMAC ============

export async function hmac256(key: Bytes, msg: Bytes): Promise<Bytes> {
  const subtle: SubtleCrypto = need_subtle();
  const k: CryptoKey = await subtle.importKey("raw", key, { name: "HMAC", hash: "SHA-256" }, false, ["sign"]);
  const mac: ArrayBuffer = await subtle.sign("HMAC", k, msg);
  return new Uint8Array(mac);
}

// ============ PID derivation ============

/**
 * derive_pid(sign_pub, dh_pub) -> PID
 * - deterministic pid derived from public material
 * - PID remains 22-char b64url (uuid-like 16 bytes)
 */
export async function derive_pid(sign_pub: IdentityKey, dh_pub: IdentityKey): Promise<PID> {
  const msg: Bytes = new Uint8Array(sign_pub.byteLength + dh_pub.byteLength);
  msg.set(sign_pub, 0);
  msg.set(dh_pub, sign_pub.byteLength);
  const h = await hash256(msg);

  // Reserve PID namespace:
  // - real identity pid: msb=0
  // - sentinel (seed/tmp): msb=1
  const uuid: UUID = new Uint8Array(h.subarray(0, 16));
  uuid[0] &= 0x7f;
  return encoding.uuid_to_b64url(uuid);
}

// ============ Identity ============

export async function generate_identity(): Promise<Identity> {
  // signing keys (ed25519)
  const sign_priv: IdentityKey = as_bytes(ed25519.utils.randomSecretKey());
  const sign_pub: IdentityKey = as_bytes(ed25519.getPublicKey(sign_priv));

  // dh keys (x25519)
  const dh_priv: IdentityKey = as_bytes(x25519.utils.randomSecretKey());
  const dh_pub: IdentityKey = as_bytes(x25519.getPublicKey(dh_priv));

  const pid: PID = await derive_pid(sign_pub, dh_pub);

  return {
    pid,
    sign_pub,
    sign_priv,
    dh_pub,
    dh_priv,
  };
}

/**
 * derive_lane_pid(tag, lane) -> PID (sentinel namespace)
 * - deterministic pid derived from transport-internal lane key
 * - sentinel pid: msb=1
 * - subspace split:
 *   - seed: 10xxxxxx
 *   - tmp : 11xxxxxx
 */
export async function derive_lane_pid(tag: "seed" | "tmp", lane: string): Promise<PID> {
  const msg: Bytes = encoding.te.encode(`${tag}\0${lane}`);
  const h: Bytes = await hash256(msg);
  const b: Bytes = h.subarray(0, 16);
  b[0] = tag === "seed" ? (b[0] & 0x3f) | 0x80 : (b[0] & 0x3f) | 0xc0;
  return encoding.bytes_to_b64url(b);
}

// ============ Bootstrap Sign/Verify ============

/**
 * bootstrap signing message:
 *   msg = INFO_BOOT || pid_bin || sign_pub || dh_pub || u64(timestamp_ms)
 */
export function bootstrap_sign(
  my_sign_priv: IdentityKey,
  pid: PID,
  my_sign_pub: IdentityKey,
  my_dh_pub: IdentityKey,
  timestamp_ms: bigint,
): Signature {
  const pid_bin: Bytes = encoding.b64url_to_bytes(pid);

  const msg: Bytes = new Uint8Array(INFO_BOOT.byteLength + pid_bin.byteLength + my_sign_pub.byteLength + my_dh_pub.byteLength + 8);
  const c: Stream = scalar.stream(msg);
  scalar.write(c, INFO_BOOT, INFO_BOOT.byteLength);
  scalar.write(c, pid_bin, pid_bin.byteLength);
  scalar.write(c, my_sign_pub, my_sign_pub.byteLength);
  scalar.write(c, my_dh_pub, my_dh_pub.byteLength);
  scalar.putu64(c, timestamp_ms);

  return as_bytes(ed25519.sign(msg, my_sign_priv));
}

/**
 * bootstrap verify:
 * - checks pid matches derive_pid(sign_pub, dh_pub)
 * - verifies signature using sign_pub
 */
export async function bootstrap_verify(pid: PID, peer_sign_pub: IdentityKey, peer_dh_pub: IdentityKey, timestamp_ms: bigint, sig: Signature): Promise<boolean> {
  const derived: PID = await derive_pid(peer_sign_pub, peer_dh_pub);
  if (derived !== pid) return false;

  const pid_bin: Bytes = encoding.b64url_to_bytes(pid);

  const msg: Bytes = new Uint8Array(INFO_BOOT.byteLength + pid_bin.byteLength + peer_sign_pub.byteLength + peer_dh_pub.byteLength + 8);
  const c: Stream = scalar.stream(msg);
  scalar.write(c, INFO_BOOT, INFO_BOOT.byteLength);
  scalar.write(c, pid_bin, pid_bin.byteLength);
  scalar.write(c, peer_sign_pub, peer_sign_pub.byteLength);
  scalar.write(c, peer_dh_pub, peer_dh_pub.byteLength);
  scalar.putu64(c, timestamp_ms);

  return ed25519.verify(sig, msg, peer_sign_pub);
}

// ============ KDF / Keys ============

export function derive_shared_secret(my_dh_priv: IdentityKey, peer_dh_pub: IdentityKey): Bytes {
  return as_bytes(x25519.getSharedSecret(my_dh_priv, peer_dh_pub));
}

/**
 * HKDF-SHA256 -> AES-GCM CryptoKey
 * - salt: rid_bin
 * - info: INFO_HDR / INFO_BODY
 */
async function derive_aead_key(shared: Bytes, rid: RID, info: Bytes): Promise<CryptoKey> {
  const subtle: SubtleCrypto = need_subtle();
  const rid_bin: Bytes = encoding.b64url_to_bytes(rid);

  const ikm: CryptoKey = await subtle.importKey("raw", shared, "HKDF", false, ["deriveKey"]);
  return subtle.deriveKey({ name: "HKDF", hash: "SHA-256", salt: rid_bin, info }, ikm, { name: "AES-GCM", length: 256 }, false, ["encrypt", "decrypt"]);
}

// ============ AEAD ============

/**
 * AEAD encrypt (AES-GCM)
 * out = iv(12) || ciphertext
 * aad: typically rid_bin
 */
export async function aead_encrypt_hdr(my_dh_priv: IdentityKey, peer_dh_pub: IdentityKey, rid: RID, plain_hdr: Bytes): Promise<Bytes> {
  const subtle: SubtleCrypto = need_subtle();
  const rid_bin: Bytes = encoding.b64url_to_bytes(rid);

  const shared: Bytes = derive_shared_secret(my_dh_priv, peer_dh_pub);
  const key: CryptoKey = await derive_aead_key(shared, rid, INFO_HDR);

  const iv: Bytes = globalThis.crypto.getRandomValues(new Uint8Array(IV_SZ));
  const ct: ArrayBuffer = await subtle.encrypt({ name: "AES-GCM", iv, additionalData: rid_bin }, key, plain_hdr);

  const out: Bytes = new Uint8Array(IV_SZ + ct.byteLength);
  out.set(iv, 0);
  out.set(new Uint8Array(ct), IV_SZ);
  return out;
}

export async function aead_decrypt_hdr(my_dh_priv: IdentityKey, peer_dh_pub: IdentityKey, rid: RID, enc_hdr: Bytes): Promise<Bytes> {
  const subtle: SubtleCrypto = need_subtle();
  const rid_bin: Bytes = encoding.b64url_to_bytes(rid);

  if (enc_hdr.byteLength < IV_SZ) throw new Error("aead_decrypt_hdr: truncated");
  const iv: Bytes = enc_hdr.subarray(0, IV_SZ);
  const ct: Bytes = enc_hdr.subarray(IV_SZ);

  const shared: Bytes = derive_shared_secret(my_dh_priv, peer_dh_pub);
  const key: CryptoKey = await derive_aead_key(shared, rid, INFO_HDR);

  const plain: ArrayBuffer = await subtle.decrypt({ name: "AES-GCM", iv, additionalData: rid_bin }, key, ct);
  return new Uint8Array(plain);
}

export async function aead_encrypt_body(my_dh_priv: IdentityKey, peer_dh_pub: IdentityKey, rid: RID, plain_body: Bytes): Promise<Bytes> {
  const subtle: SubtleCrypto = need_subtle();
  const rid_bin: Bytes = encoding.b64url_to_bytes(rid);

  const shared: Bytes = derive_shared_secret(my_dh_priv, peer_dh_pub);
  const key: CryptoKey = await derive_aead_key(shared, rid, INFO_BODY);

  const iv: Bytes = globalThis.crypto.getRandomValues(new Uint8Array(IV_SZ));
  const ct: ArrayBuffer = await subtle.encrypt({ name: "AES-GCM", iv, additionalData: rid_bin }, key, plain_body);

  const out: Bytes = new Uint8Array(IV_SZ + ct.byteLength);
  out.set(iv, 0);
  out.set(new Uint8Array(ct), IV_SZ);
  return out;
}

export async function aead_decrypt_body(my_dh_priv: IdentityKey, peer_dh_pub: IdentityKey, rid: RID, enc_body: Bytes): Promise<Bytes> {
  const subtle: SubtleCrypto = need_subtle();
  const rid_bin: Bytes = encoding.b64url_to_bytes(rid);

  if (enc_body.byteLength < IV_SZ) throw new Error("aead_decrypt_body: truncated");
  const iv = enc_body.subarray(0, IV_SZ);
  const ct = enc_body.subarray(IV_SZ);

  const shared: Bytes = derive_shared_secret(my_dh_priv, peer_dh_pub);
  const key: CryptoKey = await derive_aead_key(shared, rid, INFO_BODY);

  const plain: ArrayBuffer = await subtle.decrypt({ name: "AES-GCM", iv, additionalData: rid_bin }, key, ct);
  return new Uint8Array(plain);
}

// ============ Packet Sign / Verify ============

/**
 * sign_packet:
 *   msg = rid_bin || enc_hdr || u32(body_len) || body_hash32?
 *
 * body_hash32 is sha256(enc_body) (NOT plain body), or null if no body.
 */
export function sign_packet(my_sign_priv: IdentityKey, rid: RID, enc_hdr: Bytes, body_len: u32, body_hash32: Bytes | null): Signature {
  const rid_bin: Bytes = encoding.b64url_to_bytes(rid);
  const extra = body_hash32 ? body_hash32.byteLength : 0;

  const msg: Bytes = new Uint8Array(rid_bin.byteLength + enc_hdr.byteLength + 4 + extra);
  const c: Stream = scalar.stream(msg);
  scalar.write(c, rid_bin, rid_bin.byteLength);
  scalar.write(c, enc_hdr, enc_hdr.byteLength);
  scalar.putu32(c, body_len);
  if (body_hash32) scalar.write(c, body_hash32, body_hash32.byteLength);

  return as_bytes(ed25519.sign(msg, my_sign_priv));
}

export function verify_packet(
  peer_sign_pub: IdentityKey,
  sig: Signature,
  rid: RID,
  enc_hdr: Bytes,
  body_len: u32,
  body_hash32: Bytes | null,
): boolean {
  const rid_bin: Bytes = encoding.b64url_to_bytes(rid);
  const extra = body_hash32 ? body_hash32.byteLength : 0;

  const msg: Bytes = new Uint8Array(rid_bin.byteLength + enc_hdr.byteLength + 4 + extra);
  const c = scalar.stream(msg);
  scalar.write(c, rid_bin, rid_bin.byteLength);
  scalar.write(c, enc_hdr, enc_hdr.byteLength);
  scalar.putu32(c, body_len);
  if (body_hash32) scalar.write(c, body_hash32, body_hash32.byteLength);

  return ed25519.verify(sig, msg, peer_sign_pub);
}
