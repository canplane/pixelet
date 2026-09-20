// @xpute/runtime/mem/section.ts

/**
 * Ranges laid end to end in a linear memory: the mechanism under a kernel's
 * memory map.
 *
 * A linker script's job, and its shape. `ld` places output sections in the
 * order the script gives, each at a `. = ALIGN(n)` boundary, and answers where
 * each one landed; which sections there are, what they hold and what `n` is
 * are the script's. Nothing here knows what a range holds either — it is told
 * a base, a unit and a list of sizes.
 *
 * The central scheduler reserves the memory — one `WebAssembly.Memory` with
 * `initial == maximum`, which nothing grows — and gives it to the kernel. So
 * `memory.buffer` is one `ArrayBuffer` for the module's life: a view over it
 * stays valid, which is what lets a ring hold its words and a lane be read
 * where it lies. What the ranges are for, and how large each one is, is the
 * kernel's own policy, and nothing here reads it.
 */

import type { U32Array } from "@xpute/core/abi/array.ts";
import type { u32 } from "@xpute/core/abi/word.ts";
import { MarshalError } from "@xpute/core/status/error.ts";
import { Errno } from "@xpute/core/status/errno.spec.ts";

/** A WebAssembly page. */
export const WASM_PAGE_BYTES: u32 = 1 << 16;

/** Words a section takes in a boot block: its offset, bytes and alignment. */
export const SECTION_WORDS: u32 = 3;

export interface Section {
  offset: u32;
  bytes: u32;
  /** The boundary the section begins on, and so the one the next begins on
   * past where this one's content ends. */
  align: u32;
}

/** `n` at the next multiple of `unit`, which is where the range holding `n`
 * bytes ends and the one after it may begin. */
export function align_up(n: u32, unit: u32): u32 {
  return Math.ceil(n / unit) * unit;
}

/** What `sizes` span when laid end to end, each beginning on `unit`. The last
 * one's own rounding is in here too, which is what makes this the offset of
 * the boundary whatever follows them starts on. */
export function span_of(sizes: readonly u32[], unit: u32): u32 {
  let span = 0;
  for (const size of sizes) span += align_up(size, unit);
  return span;
}

/** Where the `n`th of `sizes` begins, laid from `base` on `unit`. */
export function nth_at(base: u32, sizes: readonly u32[], unit: u32, n: u32): u32 {
  return base + span_of(sizes.slice(0, n), unit);
}

/** Writes a section's words at `at`. */
export function write_section(words: U32Array, at: u32, s: Section): void {
  words[at] = s.offset;
  words[at + 1] = s.bytes;
  words[at + 2] = s.align;
}

/** The section whose words are at `at`. */
export function read_section(words: ArrayLike<u32>, at: u32): Section {
  return { offset: words[at], bytes: words[at + 1], align: words[at + 2] };
}

/**
 * The central scheduler's reservation: a memory of the first region count in
 * `candidates` the device will reserve, `initial == maximum` so that what it
 * hands back never grows and never detaches.
 */
export function reserve_memory(candidates: readonly u32[], region_bytes: u32): WebAssembly.Memory {
  for (const regions of candidates) {
    const pages = (regions * region_bytes) / WASM_PAGE_BYTES;
    try {
      return new WebAssembly.Memory({ initial: pages, maximum: pages });
    } catch {
      // too large for this device; try the next
    }
  }
  throw new MarshalError(Errno.ENOMEM, `memory: the device reserved none of ${candidates.join(", ")} regions`);
}
