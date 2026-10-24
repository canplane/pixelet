// @pxl/client/kernel/io/kept.ts

/**
 * The kernel's kept slots as the host holds them (host.json `kept`). The
 * kernel opens a slot by its number and reads it only where it is of the
 * slot's format, refusing anything else and writing nothing over it; which
 * storage key a slot is, what becomes of a file it refused, and how a file
 * kept before the slots carries over are decided here. Nothing kept is
 * dropped: a refused file is set aside under a key of its own, and the copy
 * read back, before its slot is emptied.
 */

import type { u32 } from "@xpute/kit/abi/word.ts";
import { b64url_to_bytes, bytes_to_b64url } from "@xpute/kit/codec/encoding.ts";

import { kept } from "@/kernel/abi/host.spec.ts";

/** Storage as the bracket gives it: a refused write keeps nothing, and does not throw. */
export interface Storage {
  get(key: string): string | null;
  set(key: string, value: string): void;
}

const KEYS: ReadonlyMap<u32, string> = new Map([
  [kept.SETTINGS, "pxl:kept:settings"],
  [kept.OBJECTS, "pxl:kept:objects"],
]);

/** A slot's storage key; none for a number the kernel does not keep. */
export function kept_key(slot: u32): string | undefined {
  return KEYS.get(slot);
}

/** Where each was kept before the slots, under a key naming it: the settings
 * by name as JSON, then for a day as their slot holds them; the objects as
 * their packet's base64 alone, the format theirs by the key. */
const SETTINGS_BEFORE = "pxl:settings";
const OBJECTS_BEFORE = "pxl:object:v15";
const OBJECTS_BEFORE_FORMAT = 15;

/** Fills a slot that holds nothing yet from what was kept before it, once:
 * a slot is never written here again once it holds anything. The settings
 * carry over as they are, and the kernel refuses the JSON (and it is set
 * aside), so they reset once; the objects carry over with their format put
 * before them. What was kept before stays where it was. */
export function migrate(s: Storage): void {
  carry(s, kept.SETTINGS, SETTINGS_BEFORE, (text) => text);
  carry(s, kept.OBJECTS, OBJECTS_BEFORE, (text) => formatted(OBJECTS_BEFORE_FORMAT, text));
}

function carry(s: Storage, slot: u32, before: string, as: (text: string) => string): void {
  const key = KEYS.get(slot)!;
  const text = s.get(before);
  if (text === null || s.get(key) !== null) return;
  s.set(key, as(text));
}

/** `text`, the base64url of a file with no format byte, with `format`
 * before its bytes; as it is where it is not base64url, for the kernel to
 * refuse and `set_aside` to keep. */
export function formatted(format: number, text: string): string {
  if (text === "") return text;
  let bytes: Uint8Array;
  try {
    bytes = b64url_to_bytes(text);
  } catch {
    return text;
  }
  const out = new Uint8Array(bytes.length + 1);
  out[0] = format;
  out.set(bytes, 1);
  return bytes_to_b64url(out);
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
    for (let n = 1; s.get(aside) !== null; n++) aside = `${key}:aside:${now_ms}-${n}`;
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

/** The snapshot's `kept_refused`: each slot set aside once, and the kernel
 * told it is empty (`emptied`). */
export function follow_refused(refused: readonly u32[], s: Storage, emptied: (slot: u32) => void): void {
  for (const slot of emptying) if (!refused.includes(slot)) emptying.delete(slot);
  for (const slot of refused) {
    if (emptying.has(slot) || !set_aside(s, slot, Date.now())) continue;
    emptying.add(slot);
    emptied(slot);
  }
}
