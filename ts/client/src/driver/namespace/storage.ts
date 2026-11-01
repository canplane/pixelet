// @pxl/client/driver/namespace/storage.ts

/**
 * STORAGE's backend: the kernel's storage files (host.json `storage`), the
 * settings, in the platform's storage. The kernel refuses a file not of its
 * slot's format and writes nothing over it; here is decided each slot's key,
 * what becomes of a refused file, and what carries over from before the
 * slots. Nothing kept is dropped: a refused file is set aside, and read back,
 * before its slot empties.
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import type { u32 } from "@xpute/kit/abi/word.ts";
import { td, te } from "@xpute/kit/codec/encoding.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { FileError } from "@xpute/host/ipc/sys.ts";

import { Command, encode_fs_truncated } from "@/abi/cmd.spec.ts";
import { storage } from "@/abi/host.spec.ts";
import { bracket } from "@/bracket.tsx";
import { HOST } from "@/host/mount.ts";
import * as port from "@/abi/port.spec.ts";

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

const KEYS: ReadonlyMap<u32, string> = new Map([
  [storage.SETTINGS, "pxl:kept:settings"],
]);

/** A slot's storage key; none for a number the kernel does not keep. */
export function storage_key(slot: u32): string | undefined {
  return KEYS.get(slot);
}

/** Where the settings were kept before their slot, by name as JSON, then for
 * a day as their slot holds them. */
const SETTINGS_BEFORE = "pxl:settings";

/** Before the kernel reads a slot: an empty slot is filled from what was kept
 * before it, once, and that is left where it was. The settings carry over as
 * they are, so the kernel refuses the JSON and they reset once. */
export function migrate(s: Storage = STORAGE): void {
  carry(s, storage.SETTINGS, SETTINGS_BEFORE);
}

function carry(s: Storage, slot: u32, before: string): void {
  const key = KEYS.get(slot)!;
  const text = s.get(before);
  if (text === null || s.get(key) !== null) return;
  s.set(key, text);
}

/** Sets a refused slot's file aside under `<key>:aside:<ms>` and empties
 * the slot. False, the slot as it was, for a slot the kernel does not keep or
 * where the copy did not read back: storage that refuses the write must not
 * see the file emptied. */
export function set_aside(s: Storage, slot: u32, now_ms: number): boolean {
  const key = KEYS.get(slot);
  if (key === undefined) return false;
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

/** The slots set aside and not yet seen read again, so a snapshot that
 * still names one before the kernel hears it was emptied sets nothing aside
 * twice. */
const emptying = new Set<u32>();

/** The kernel told a slot is empty (FS_TRUNCATED), after the completion
 * ring is read: the doorbell refuses a write while it reads. */
function storage_emptied(slot: u32): void {
  queueMicrotask(() =>
    HOST.send(
      port.SHELL,
      Command.FS_TRUNCATED,
      encode_fs_truncated(HOST.doorbell, slot),
    ),
  );
}

/** The snapshot's `storage_refused`: each slot set aside once, and the kernel
 * told it is empty (`emptied`). */
export function follow_refused(
  refused: readonly u32[],
  emptied: (slot: u32) => void = storage_emptied,
  s: Storage = STORAGE,
): void {
  for (const slot of emptying) {
    if (!refused.includes(slot)) emptying.delete(slot);
  }
  for (const slot of refused) {
    if (emptying.has(slot) || !set_aside(s, slot, Date.now())) continue;
    emptying.add(slot);
    emptied(slot);
  }
}

/** STORAGE's object `number` is a storage file, under the key `storage_key` gives it. */
function key_of(number: bigint): string {
  const key = number <= 0xffff_ffffn ? storage_key(Number(number)) : undefined;
  if (key === undefined) throw new FileError(Errno.ENOENT);
  return key;
}

/** STORAGE's object `number`, whole; ENOENT where nothing is kept. */
export function open_storage(number: bigint): U8Array {
  const value = STORAGE.get(key_of(number));
  if (value === null) throw new FileError(Errno.ENOENT);
  return te.encode(value);
}

/** What the kernel wrote to STORAGE's object `number`, kept whole. */
export function commit_storage(number: bigint, bytes: U8Array): void {
  STORAGE.set(key_of(number), td.decode(bytes));
}
