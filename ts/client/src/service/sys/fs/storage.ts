// @pxl/client/service/sys/fs/storage.ts

/**
 * The backend of the core's `path.STORAGE` files (file.json5), which hold
 * the settings, in the platform's storage. The core hands and takes a file's
 * bytes, while the platform keeps text, so each file is kept as the base64url
 * of its bytes. Text that is not base64url reads as `bad-message`, which the core
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
import { ComponentError } from "@xpute/link";
import {
  ERROR_CODE_BAD_MESSAGE,
  ERROR_CODE_IO,
  ERROR_CODE_NO_ENTRY,
} from "@xpute/link/abi/fs/types.enum.ts";

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
  if (!is_b64url(text)) throw new ComponentError(ERROR_CODE_BAD_MESSAGE);
  try {
    return b64url_to_bytes(text);
  } catch {
    throw new ComponentError(ERROR_CODE_BAD_MESSAGE);
  }
}

/** Each stored file's key, by its id under STORAGE (file.json5 `path`). */
const KEYS: ReadonlyMap<u32, string> = new Map([
  [path.STORAGE.SETTINGS.ID, "pxl:settings"],
]);

/** Whether `key` holds a storage file or one set aside. These are the keys a
 * page copies to its core's worker (runtime/worker.ts). */
export function is_storage_key(key: string): boolean {
  for (const k of KEYS.values()) {
    if (key === k || key.startsWith(`${k}:aside:`)) return true;
  }
  return false;
}

/** Copies what `key` holds to `<key>:aside:<now_ms>`, reads the copy back, and
 * then empties `key`. When the copy does not read back, the storage refused
 * the write, so `key` is left as it was and the set-aside fails `io`: the file
 * is never emptied without a copy. */
function set_aside_key(s: Storage, key: string, now_ms: number): void {
  const text = s.get(key);
  if (text !== null && text !== "") {
    let aside = `${key}:aside:${now_ms}`;
    for (let n = 1; s.get(aside) !== null; n++) {
      aside = `${key}:aside:${now_ms}-${n}`;
    }
    s.set(aside, text);
    if (s.get(aside) !== text) throw new ComponentError(ERROR_CODE_IO);
  }
  s.set(key, "");
}

export function set_aside(s: Storage, slot: u32, now_ms: number): void {
  set_aside_key(s, key_of(slot), now_ms);
}

function key_of(slot: u32): string {
  const key = KEYS.get(slot);
  if (key === undefined) throw new ComponentError(ERROR_CODE_NO_ENTRY);
  return key;
}

export function open_storage(slot: u32, s: Storage = STORAGE): U8Array {
  const text = s.get(key_of(slot));
  if (text === null) throw new ComponentError(ERROR_CODE_NO_ENTRY);
  return bytes_of(text);
}

/** Writes `bytes` as the file in `slot`, and fails `io` where the platform
 * refused the write: what it holds then does not read back. */
export function commit_storage(
  slot: u32,
  bytes: U8Array,
  s: Storage = STORAGE,
): void {
  const key = key_of(slot);
  const text = bytes_to_b64url(bytes);
  s.set(key, text);
  if (s.get(key) !== text) throw new ComponentError(ERROR_CODE_IO);
}
