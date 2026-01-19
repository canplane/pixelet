// @xpute/core/net/kit/guard.ts

import { as_bytes } from "@xpute/spec/lib/kit/array.ts";

import type { Handle, Key } from "@xpute/spec/net/xport/mod.ts";
import type { Message } from "@xpute/spec/net/xport/central.ts";

import { drop, type DropHook } from "./drop.ts";

// ============ Limits ============

export const MAX_HANDLE_LEN = 256;
export const MAX_KEY_LEN = 512;
export const MAX_BLOB_LEN = 8 * 1024 * 1024; // 8 MiB

// HTTPPlug limits (분리해도 되고 여기 있어도 됨)
export const MAX_BOX_ENTRIES = 2048;
export const BOX_TTL_MS = 60_000; // 60s

// ============ Validate ============

function die(hook: DropHook | null, tag: string, why: string): null {
  if (hook) hook(tag, { why });
  return null;
}

export function decode_handle(x: unknown, tag: string): Handle | null {
  if (typeof x !== "string") return die(drop, tag, "bad handle type");
  if (!x.length) return die(drop, tag, "empty handle");
  if (x.length > MAX_HANDLE_LEN) return die(drop, tag, "handle too long");
  return x;
}

export function decode_key(x: unknown, tag: string): Key | null {
  if (typeof x !== "string") return die(drop, tag, "bad key type");
  if (!x.length) return die(drop, tag, "empty key");
  if (x.length > MAX_KEY_LEN) return die(drop, tag, "key too long");
  return x;
}

export function ok_blob_len(n: number, tag: string): boolean {
  if (n <= 0) {
    drop(tag, { why: "empty blob" });
    return false;
  }
  if (n > MAX_BLOB_LEN) {
    drop(tag, { why: "blob too large" });
    return false;
  }
  return true;
}

// ============ Codec ============

export function decode_wire_message(msg: any, drop_tag: string = ""): Message | null {
  const die = (why: string): null => {
    drop(drop_tag, { why });
    return null;
  };

  if (!msg || typeof msg !== "object") return die("msg not object");

  const key = msg.key;
  const has_blob = msg.has_blob;

  if (typeof key !== "string") return die("bad key");
  if (!key.length) return die("empty key");
  if (key.length > MAX_KEY_LEN) return die("key too long");
  if (has_blob !== 0 && has_blob !== 1) return die("bad has_blob");

  const enc_hdr = as_bytes(msg.enc_hdr);
  const sig = as_bytes(msg.sig);

  if (!enc_hdr.byteLength) return die("empty enc_hdr");
  // NOTE: sig may be empty in pre-auth handshake (HELLO SYN/RETRY).

  return { key, has_blob, enc_hdr, sig };
}
