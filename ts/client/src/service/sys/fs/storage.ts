// @pxl/client/service/sys/fs/storage.ts

/**
 * The backend of the core's STORAGE files (file.json `storage`), which hold
 * the settings, in the platform's storage. The core hands and takes a file's
 * bytes, while the platform keeps text, so each file is kept as the base64url
 * of its bytes. Text that is not base64url reads as EBADMSG, which the core
 * refuses. The core also refuses a file that is not in its slot's format, and
 * writes nothing over it. This module decides each slot's key and what becomes
 * of a refused file. Nothing kept is dropped: a refused file is set aside, and
 * read back, before its slot is emptied.
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import type { u32 } from "@xpute/kit/abi/word.ts";
import {
  b64url_to_bytes,
  bytes_to_b64url,
  is_b64url,
} from "@xpute/kit/codec/encoding.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { FileError } from "@xpute/runtime";

import { path } from "@/abi/file.spec.ts";
import * as local from "@/service/platform/storage.ts";

/** A write the platform refuses keeps nothing and does not throw. */
export interface Storage {
  get(key: string): string | null;
  set(key: string, val: string): void;
}

export let STORAGE: Storage = local;

/** A page's worker cannot reach the page's storage, so the core's worker keeps
 * a copy of it that writes back to the page (runtime/worker.ts). */
export function set_storage(s: Storage): void {
  STORAGE = s;
}

function bytes_of(text: string): U8Array {
  if (!is_b64url(text)) throw new FileError(Errno.EBADMSG);
  try {
    return b64url_to_bytes(text);
  } catch {
    throw new FileError(Errno.EBADMSG);
  }
}

/** Each stored file's key, by its id under STORAGE (file.json `path`). */
const KEYS: ReadonlyMap<u32, string> = new Map([
  [path.STORAGE.SETTINGS.ID, "pxl:settings"],
]);

/** Whether `key` holds a storage file or one set aside. These are the keys a
 * page copies to its core's worker (runtime/open.ts). */
export function is_storage_key(key: string): boolean {
  for (const k of KEYS.values()) {
    if (key === k || key.startsWith(`${k}:aside:`)) return true;
  }
  return false;
}

/** Copies what `key` holds to `<key>:aside:<now_ms>`, reads the copy back, and
 * then empties `key`. When the copy does not read back, the storage refused
 * the write, so `key` is left as it was and the answer is false: the file is
 * never emptied without a copy. */
function set_aside_key(s: Storage, key: string, now_ms: number): boolean {
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

export function set_aside(s: Storage, slot: u32, now_ms: number): boolean {
  const key = KEYS.get(slot);
  return key !== undefined && set_aside_key(s, key, now_ms);
}

function key_of(slot: u32): string {
  const key = KEYS.get(slot);
  if (key === undefined) throw new FileError(Errno.ENOENT);
  return key;
}

export function open_storage(slot: u32, s: Storage = STORAGE): U8Array {
  const text = s.get(key_of(slot));
  if (text === null) throw new FileError(Errno.ENOENT);
  return bytes_of(text);
}

export function commit_storage(
  slot: u32,
  bytes: U8Array,
  s: Storage = STORAGE,
): void {
  s.set(key_of(slot), bytes_to_b64url(bytes));
}
