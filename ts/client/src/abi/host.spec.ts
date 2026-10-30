// @pxl/client/abi/host.spec.ts
//
// GENERATED from spec/client/kernel/sys/io/host.json
//
// The kernel's own system calls, numbered beside xpute's (xpute/spec/ipc/sys.json,
// pxl-kernel sys/io/host.rs, ts/client/src/driver/text.ts) and made as hostcalls, answered
// before they return; and the range of the stream xpute's calls are recorded in, which
// a hostcall's record lies past while it runs.

import type { u32 } from "@xpute/kit/abi/word.ts";

/** The stream's range, laid out by the kernel's map. */
export const stream = {
  /** The range's size. A record names what it moves rather than carrying it, so
   * a turn's records are words, and a hostcall's text bytes the most of what it holds. */
  BYTES: 65536 as number,
} as const;

/** The kernel's calls, each with xpute's PROGRAM bit and made as a hostcall; a record's call, then its arguments. */
export const op = {
  /** weight, px (f32 bits), max width (f32 bits, 0 for one line), at, lines, then the
   * text's bytes: the text in the platform's own family at the weight and size, broken
   * into lines no wider than the width where it can be; at `at`, how many lines, then
   * a line's end (a byte offset) and its width (f32) apiece, `lines` of them at most */
  TEXT_LAYOUT: 2147483649 as number,
  /** weight, px (f32 bits), color (0xRRGGBBAA), at, width, height, x (f32 bits), then
   * the text's bytes: one line drawn in the platform's own family, starting at x with
   * the middle of its em box on the middle row, into `width` by `height` pixels at
   * `at`, RGBA a byte a channel, top row first, alpha not premultiplied */
  TEXT_RASTER: 2147483650 as number,
} as const;

/** TEXT_LAYOUT's arguments, a word each after its call, then the text's bytes at
 * `WORDS`: the weight, px and max width as f32 bits, where its answer goes and
 * how many lines it may hold. */
export const LAYOUT = {
  weight: 0 as number,
  px: 1 as number,
  max_width: 2 as number,
  at: 3 as number,
  lines: 4 as number,
  WORDS: 5 as number,
} as const;

/** TEXT_LAYOUT's answer at its `at`: how many lines, then `LINE` apiece. */
export const LAYOUT_ANSWER = {
  cnt: 0 as number,
  WORDS: 1 as number,
} as const;

/** A line of TEXT_LAYOUT's answer: its end, a byte offset, and its width as f32 bits. */
export const LINE = {
  end: 0 as number,
  width: 1 as number,
  WORDS: 2 as number,
} as const;

/** TEXT_RASTER's arguments, a word each after its call, then the text's bytes at
 * `WORDS`: the weight and px as f32 bits, the color, where its pixels go, their
 * width and height, and x as f32 bits. */
export const RASTER = {
  weight: 0 as number,
  px: 1 as number,
  color: 2 as number,
  at: 3 as number,
  width: 4 as number,
  height: 5 as number,
  x: 6 as number,
  WORDS: 7 as number,
} as const;

/** The roots the kernel opens files below (xpute's OPENAT). Below STORAGE a root is a
 * bake, numbered by its kind (pxl-spec bake/published.spec.rs). Below every root a path is
 * an object's number in decimal, its one name; what that object is called and where, a
 * tile's version and URL, a storage key, is what the host (ts/client/src/driver/namespace/)
 * binds the root to, `@xpute/host` carrying the open there. */
export const root = {
  /** The platform's storage; a path is a kept slot's number in decimal (`kept`); where the platform holds a slot is the host's choice. */
  STORAGE: 256 as number,
  /** The shipped assets; a path is a load's number in decimal, the catalog's (`CATALOG`) or a file it names (sys/io/loader.rs). */
  ASSET: 257 as number,
} as const;

/** Below the earth's root, the Earth's picture whole (pxl-spec route.json's `TILE_SPHERE`),
 * under the earth's own version. Any other number below a bake's root is a tile's subject
 * (pxl-spec space/layer/tile.rs), which is never 0: a tile's key ends in a 1 within its
 * high half. */
export const SPHERE: u32 = 0 as number;

/** Below the asset root, the catalog the host plugs in (pxl-spec asset/catalog.rs): the
 * file each of the asset specs' references names. Any other number below it is a file,
 * its row in the asset ingest's table counted from 1 (spec/spec/asset/ingested.json). */
export const CATALOG: u32 = 0 as number;

/** The files the kernel keeps below STORAGE, by slot. What a slot holds is its
 * owner's, written whole, its slot's `format` first (pxl-kernel sys/io/file.rs). */
export const kept = {
  /** The settings' overrides (domain/view/setting/store.rs). */
  SETTINGS: 0 as number,
  /** The placed objects (domain/layer/object/repository.rs). */
  OBJECTS: 1 as number,
} as const;

/** Each slot's format, the first byte of what it holds. The kernel reads a file of its
 * slot's format and refuses any other, reading none of it and never writing over it, as
 * a driver refuses a volume whose superblock it does not know; what to do with the file
 * is the host's. Bump one whenever a change makes an older file mean something other
 * than it meant when it was written. */
export const format = {
  SETTINGS: 1 as number,
  OBJECTS: 15 as number,
} as const;
