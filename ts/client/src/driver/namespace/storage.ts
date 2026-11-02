// @pxl/client/driver/namespace/storage.ts

/**
 * STORAGE's backend: the kernel's storage files (host.json `storage`), the
 * settings, in the platform's storage. The kernel hands and takes a file's
 * bytes; the platform keeps text, so each is kept as its bytes' base64url
 * (`kept`, `bytes_of`), and text that is none is no file of bytes
 * (EBADMSG), which the kernel refuses. The kernel refuses a file not of its
 * slot's format and writes nothing over it; here is decided each slot's key
 * and what becomes of a refused file. Nothing kept is dropped: a refused file
 * is set aside, and read back, before its slot empties.
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import type { u32 } from "@xpute/kit/abi/word.ts";
import {
  b64url_to_bytes,
  bytes_to_b64url,
  is_b64url,
} from "@xpute/kit/codec/encoding.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { FileError } from "@xpute/host/ipc/sys.ts";

import { storage } from "@/abi/host.spec.ts";
import { bracket } from "@/bracket.tsx";

/** Storage as the bracket gives it: a refused write keeps nothing, and does not throw. */
export interface Storage {
  get(key: string): string | null;
  set(key: string, value: string): void;
}

/** The bracket's, read at each call: the bracket is set after this loads. */
export const STORAGE: Storage = {
  get: (key) => bracket.storage.get(key),
  set: (key, value) => bracket.storage.set(key, value),
};

/** A file's bytes as storage keeps them, as text. */
export function text_of(bytes: U8Array): string {
  return bytes_to_b64url(bytes);
}

/** The bytes `text` keeps (`kept`); EBADMSG for text that keeps none. */
export function bytes_of(text: string): U8Array {
  if (!is_b64url(text)) throw new FileError(Errno.EBADMSG);
  try {
    return b64url_to_bytes(text);
  } catch {
    throw new FileError(Errno.EBADMSG);
  }
}

const KEYS: ReadonlyMap<u32, string> = new Map([
  [storage.SETTINGS, "pxl:storage:settings"],
]);

/** A slot's storage key; none for a number the kernel does not keep. */
export function storage_key(slot: u32): string | undefined {
  return KEYS.get(slot);
}

/** Sets what `key` holds aside under `<key>:aside:<ms>` and empties it.
 * False, the key as it was, where the copy did not read back: storage that
 * refuses the write must not see the file emptied. */
export function set_aside_key(
  s: Storage,
  key: string,
  now_ms: number,
): boolean {
  const text = s.get(key);
  if (text !== null && text !== "") {
    let aside = `${key}:aside:${now_ms}`;
    for (let n = 1; s.get(aside) !== null; n++) {
      aside = `${key}:aside:${now_ms}-${n}`;
    }
    s.set(aside, text);
    if (s.get(aside) !== text) return false;
  }
  s.set(key, "");
  return true;
}

/** Sets a refused slot's file aside and empties the slot (`set_aside_key`);
 * false for a slot the kernel does not keep. */
export function set_aside(s: Storage, slot: u32, now_ms: number): boolean {
  const key = KEYS.get(slot);
  return key !== undefined && set_aside_key(s, key, now_ms);
}

/** STORAGE's object `number` is a storage file, under the key `storage_key` gives it. */
function key_of(number: bigint): string {
  const key = number <= 0xffff_ffffn ? storage_key(Number(number)) : undefined;
  if (key === undefined) throw new FileError(Errno.ENOENT);
  return key;
}

/** STORAGE's object `number`, whole; ENOENT where nothing is kept. */
export function open_storage(number: bigint, s: Storage = STORAGE): U8Array {
  const text = s.get(key_of(number));
  if (text === null) throw new FileError(Errno.ENOENT);
  return bytes_of(text);
}

/** What the kernel wrote to STORAGE's object `number`, kept whole. */
export function commit_storage(
  number: bigint,
  bytes: U8Array,
  s: Storage = STORAGE,
): void {
  s.set(key_of(number), text_of(bytes));
}
