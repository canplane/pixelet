// @pxl/client/sys/fs/storage.ts

/**
 * STORAGE's backend: the kernel's storage files (file.json `storage`), the
 * settings, in the platform's storage. The kernel hands and takes a file's
 * bytes; the platform keeps text, so each is kept as its bytes' base64url
 * (`bytes_of`), and text that is none is no file of bytes
 * (EBADMSG), which the kernel refuses. The kernel refuses a file not of its
 * slot's format and writes nothing over it; here is decided each slot's key
 * and what becomes of a refused file. Nothing kept is dropped: a refused file
 * is set aside, and read back, before its slot empties.
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import {
  b64url_to_bytes,
  bytes_to_b64url,
  is_b64url,
} from "@xpute/kit/codec/encoding.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { FileError } from "@xpute/host/ipc/sys.ts";

import { storage } from "@/abi/file.spec.ts";
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

/** The bytes `text` keeps; EBADMSG for text that keeps none. */
function bytes_of(text: string): U8Array {
  if (!is_b64url(text)) throw new FileError(Errno.EBADMSG);
  try {
    return b64url_to_bytes(text);
  } catch {
    throw new FileError(Errno.EBADMSG);
  }
}

/** Each storage file's key, by its path below STORAGE: its slot in decimal. */
const KEYS: ReadonlyMap<string, string> = new Map([
  [String(storage.SETTINGS), "pxl:settings"],
]);

/** Sets what `key` holds aside under `<key>:aside:<ms>` and empties it.
 * False, the key as it was, where the copy did not read back: storage that
 * refuses the write must not see the file emptied. */
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

/** Sets the refused file `path` aside and empties its slot (`set_aside_key`);
 * false for one the kernel does not keep. */
export function set_aside(s: Storage, path: string, now_ms: number): boolean {
  const key = KEYS.get(path);
  return key !== undefined && set_aside_key(s, key, now_ms);
}

/** The key the file `path` is kept under; ENOENT for one the kernel does not keep. */
function key_of(path: string): string {
  const key = KEYS.get(path);
  if (key === undefined) throw new FileError(Errno.ENOENT);
  return key;
}

/** The file `path`, whole; ENOENT where nothing is kept. */
export function open_storage(path: string, s: Storage = STORAGE): U8Array {
  const text = s.get(key_of(path));
  if (text === null) throw new FileError(Errno.ENOENT);
  return bytes_of(text);
}

/** What the kernel wrote to the file `path`, kept whole. */
export function commit_storage(
  path: string,
  bytes: U8Array,
  s: Storage = STORAGE,
): void {
  s.set(key_of(path), bytes_to_b64url(bytes));
}
